//! 用户配置 —— 目前只存"手动指定的 Loomy 位置"。
//!
//! # 为什么需要它
//!
//! 自动探测（`core::paths`）在本机实测能扫到默认安装。但它有两个盖不住的
//! 场景：
//!
//! 1. **Loomy 装在非默认位置**（绿色版、改过安装目录、多用户共享机）
//! 2. **从没运行过 Loomy** —— leveldb 与 `auth-session.json` 都还没生成
//!
//! 这时自动探测会返回空，用户看着一个"未找到 Loomy"的界面无路可走。
//! 所以加这条手动通道：探测失败时让用户指一个目录，存下来，之后优先用。
//!
//! # 为什么存"用户选的原目录"而不是"解析后的具体文件"
//!
//! 用户选的可能有三种形态，都该接受：
//!
//! ```text
//! <安装根>               ← 最常见（C:\Users\Public\Loomy\<安装ID>）
//! <安装根>\userData      ← 有人在文件管理器里点深了一层
//! <某处>\auth-session.json  ← 直接指着文件
//! ```
//!
//! 存原值、每次读时重新解析，比"存解析结果"更稳 —— 用户之后可能把
//! 路径点错，改正了就该立刻生效，不用清配置。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::error::Result;
use crate::core::paths;

/// 持久化在 `data/config.json`。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// 用户手动指定的 Loomy 位置（原样存储，见模块注释）。
    ///
    /// `None` / 空字符串 = 只用自动探测。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loomy_path: Option<String>,
}

/// 配置文件路径。
pub fn config_file() -> PathBuf {
    paths::data_dir().join("config.json")
}

impl AppConfig {
    /// 读配置（用真实路径）。
    pub fn load() -> Self {
        Self::load_from(&config_file())
    }

    /// 写配置（用真实路径）。
    pub fn save(&self) -> Result<()> {
        self.save_to(&config_file())
    }

    /// 从**指定**文件读 —— 测试用这个，不碰真实配置。
    ///
    /// # 为什么参数化而不是用环境变量做出口
    ///
    /// 一开始试过 `LOOMY_MGR_CONFIG_DIR` 环境变量。但 **cargo 默认并行跑测试**，
    /// 环境变量是**进程全局**的：一条测试 `set_var`、另一条 `remove_var`，
    /// 相互干扰 —— 结果仍然是往真实配置里写了垃圾
    ///（实测留下了 `D:\temp\prio-31720\manual-root`）。
    ///
    /// 参数化则完全没有共享状态：每条测试用自己的临时文件，互不影响。
    /// 这也是"把纯逻辑与环境依赖分开"的一般做法。
    pub fn load_from(path: &std::path::Path) -> Self {
        // 配置坏了不该让整个应用起不来 —— 退回默认（只用自动探测），
        // 用户重新指定一次即可
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// 写到**指定**文件 —— 测试用这个。
    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        // 与账号库一样用原子替换：写一半崩掉会留下坏 JSON
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// 清掉手动指定，回到纯自动探测。
    pub fn clear_loomy_path() -> Result<Self> {
        let mut c = Self::load();
        c.loomy_path = None;
        c.save()?;
        Ok(c)
    }
}

/// 把用户给的路径解析成安装根。
///
/// 接受三种形态（见模块注释），返回 `None` 表示这个路径下找不到
/// `auth-session.json`。
///
/// # 为什么要单独一个函数
///
/// 用户的选择必须**当场校验**。如果只存下路径、等到切换时才发现不对，
/// 用户会在"点了切换、失败、再回来改"之间来回 —— 而错在哪一步并不明显。
/// 这里让命令层能在保存时立刻给出"这个目录里没有 Loomy 登录态"。
pub fn resolve_root(input: &str) -> Option<paths::LoomyRoot> {
    let p = PathBuf::from(input.trim());
    if p.as_os_str().is_empty() {
        return None;
    }

    // 形态 ①：直接给了 auth-session.json
    if p.is_file() {
        if p.file_name().is_some_and(|n| n == "auth-session.json") {
            let root = p.parent()?.parent()?.to_path_buf(); // .../<安装ID>/userData/..
            let auth_file = p.clone();
            if auth_file.is_file() {
                return Some(paths::LoomyRoot { root, auth_file });
            }
        }
        return None;
    }

    if !p.is_dir() {
        return None;
    }

    // 形态 ②：给了 <安装根>\userData
    if p.file_name().is_some_and(|n| n == "userData") {
        let auth_file = p.join("auth-session.json");
        if auth_file.is_file() {
            let root = p.parent()?.to_path_buf();
            return Some(paths::LoomyRoot { root, auth_file });
        }
    }

    // 形态 ③：给了安装根
    let auth_file = p.join("userData").join("auth-session.json");
    if auth_file.is_file() {
        return Some(paths::LoomyRoot {
            root: p.clone(),
            auth_file,
        });
    }

    // 形态 ④：给了 `C:\Users\Public\Loomy`（**容器**，里面按安装 ID 分子目录）。
    // 这是很容易发生的一次点击 —— 用户打开这个目录看到的就是一个子目录，
    // 很可能直接选它。这里有子目录就取第一个有效的，替用户省一步。
    if let Ok(entries) = std::fs::read_dir(&p) {
        let mut cands: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|c| c.is_dir())
            .collect();
        // 排序保证多次运行结果一致（readdir 顺序不保证）
        cands.sort();
        for c in cands {
            let auth_file = c.join("userData").join("auth-session.json");
            if auth_file.is_file() {
                return Some(paths::LoomyRoot { root: c, auth_file });
            }
        }
    }

    None
}

/// 找一个可能的 Loomy 安装目录供用户确认（用于"自动探测失败但能猜"的场景）。
///
/// 只是**猜测**，用来预填给用户，绝不直接采用。
pub fn guess_candidates() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(public) = std::env::var_os("PUBLIC") {
        let p = Path::new(&public).join("Loomy");
        if p.is_dir() {
            out.push(p.display().to_string());
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        for n in ["Loomy", "loomy"] {
            let p = Path::new(&appdata).join(n);
            if p.is_dir() {
                let s = p.display().to_string();
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 造一个假的 Loomy 安装目录（含 userData/auth-session.json）。
    fn fake_root(base: &Path, id: &str) -> PathBuf {
        let root = base.join(id);
        fs::create_dir_all(root.join("userData")).unwrap();
        fs::write(root.join("userData").join("auth-session.json"), "{}").unwrap();
        root
    }

    #[test]
    fn resolve_accepts_install_root() {
        let base = std::env::temp_dir().join(format!("cfg-test-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = fake_root(&base, "abc123");

        let r = resolve_root(&root.display().to_string()).expect("应当解析出安装根");
        assert_eq!(r.root, root);
        assert!(r.auth_file.ends_with("auth-session.json"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_accepts_userdata_subdir() {
        let base = std::env::temp_dir().join(format!("cfg-test-ud-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = fake_root(&base, "abc123");

        // 用户点深了一层
        let r = resolve_root(&root.join("userData").display().to_string())
            .expect("给 userData 子目录也应当能解析");
        assert_eq!(r.root, root);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_accepts_auth_file_directly() {
        let base = std::env::temp_dir().join(format!("cfg-test-file-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = fake_root(&base, "abc123");

        let r = resolve_root(
            &root
                .join("userData")
                .join("auth-session.json")
                .display()
                .to_string(),
        )
        .expect("直接给 auth-session.json 也应当能解析");
        assert_eq!(r.root, root);

        let _ = fs::remove_dir_all(&base);
    }

    /// 给**容器**目录（`...\Loomy`，里面按安装 ID 分子目录）时，
    /// 应当自动下探一层找到有效安装 —— 这是一次很容易发生的误点。
    #[test]
    fn resolve_descends_into_container_dir() {
        let base = std::env::temp_dir().join(format!("cfg-test-cont-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let _ = fake_root(&base, "abc123");

        let r = resolve_root(&base.display().to_string()).expect("给容器目录应当自动下探找到安装");
        assert_eq!(r.root, base.join("abc123"));

        let _ = fs::remove_dir_all(&base);
    }

    /// 指到无关目录必须返回 None（而不是给出一个能"保存成功"的假结果）。
    #[test]
    fn resolve_rejects_unrelated_dir() {
        let base = std::env::temp_dir().join(format!("cfg-test-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("not-loomy")).unwrap();

        assert!(resolve_root(&base.display().to_string()).is_none());
        assert!(resolve_root("").is_none());
        assert!(resolve_root("   ").is_none());
        assert!(resolve_root(r"Z:\definitely\does\not\exist").is_none());

        let _ = fs::remove_dir_all(&base);
    }

    /// 配置读写往返。
    #[test]
    fn config_roundtrip() {
        // 用参数化的 load_from / save_to，**完全不碰全局状态**。
        //
        // 两条弯路都走过，记在这里避免重犯：
        //  ① 直接读写真实配置再"还原" —— 还原的是"测试开始时的快照"，
        //     若那条快照已被别的测试改过，就把 `X:\test\path` 留给了用户
        //  ② 改用 `LOOMY_MGR_CONFIG_DIR` 环境变量 —— cargo 默认并行跑测试，
        //     环境变量是进程全局的，两条测试 set/remove 互相干扰，照样污染
        //     （实测留下了 `D:\temp\prio-31720\manual-root`）
        //
        // 结论：想隔离就不能依赖进程全局状态，把路径参数化才可靠。
        let tmp = std::env::temp_dir().join(format!("cfg-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("config.json");

        let mut c = AppConfig::load_from(&file);
        assert!(c.loomy_path.is_none(), "初始应当没有手动路径");
        c.loomy_path = Some(r"X:\test\path".into());
        c.save_to(&file).unwrap();

        let back = AppConfig::load_from(&file);
        assert_eq!(back.loomy_path.as_deref(), Some(r"X:\test\path"));

        // 清空后重新读，应当回到 None（且文件里不该留 null 字段）
        AppConfig { loomy_path: None }.save_to(&file).unwrap();
        assert!(AppConfig::load_from(&file).loomy_path.is_none());
        let raw = std::fs::read_to_string(&file).unwrap();
        assert!(!raw.contains("loomyPath"), "None 不该留在文件里：{raw}");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// `loomy_path` 为 None 时不该写进 JSON（配置文件里留个 null 会让人困惑）。
    #[test]
    fn empty_config_omits_field() {
        let c = AppConfig { loomy_path: None };
        let j = serde_json::to_string(&c).unwrap();
        assert!(!j.contains("loomyPath"), "None 不该序列化出来：{j}");
        // 反序列化缺失字段要能工作
        let back: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(back.loomy_path.is_none());
    }
}
