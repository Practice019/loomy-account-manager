//! 定位本机 Loomy 的安装与登录态位置。
//!
//! # 为什么单独一个模块
//!
//! "去哪儿读写登录态"这个知识散落在多处会让它们**各自漂移** ——
//! 而登录态分布在三个位置（见 `session` 模块），少写一处就是
//! "切换不生效"那个 bug。集中在这里，只有一份定义。
//!
//! # 三个位置（本机实测）
//!
//! ```text
//! ① C:\Users\Public\Loomy\<安装ID>\userData\auth-session.json
//!      electron-store，主进程启动时读它
//! ② C:\Users\Public\Loomy\<安装ID>\opencode\opencode.json
//!      provider.imodel.options.apiKey（useSessionAuth 时可为空）
//! ③ %APPDATA%\Loomy\Local Storage\leveldb\*.log
//!      Chromium localStorage，渲染进程读它 **并覆盖 ①**
//! ```
//!
//! ③ 是最容易被漏掉的一处，也是本工具在"切换不生效"上踩过的坑：
//! 只写 ①② 时，Loomy 下次启动会用 ③ 里的旧值把 ① 覆盖回去。

use std::path::{Path, PathBuf};

/// 一处 Loomy 安装。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoomyRoot {
    /// 安装根（`C:\Users\Public\Loomy\<安装ID>`）。
    pub root: PathBuf,
    /// `auth-session.json` 的完整路径。
    pub auth_file: PathBuf,
}

/// 找出本机所有 Loomy 安装。
///
/// # 解析顺序
///
/// ```text
/// ① 用户手动指定的路径（配置里）—— 优先级最高
/// ② C:\Users\Public\Loomy\<安装ID>\userData\auth-session.json   ← 默认安装位置
/// ③ %APPDATA%\Loomy\auth-session.json                          ← 旧版/便携版
/// ```
///
/// ① 放最前是因为：**用户明确告诉我们的，胜过我们猜的**。
/// 自动探测在"装在非默认位置"或"从没运行过"时会返回空，那时用户手动
/// 指定的就是唯一可行路径；若把它排在后面，用户指定了也没用。
///
/// 判定标准始终是**存在 `userData/auth-session.json`** —— 只看目录名会把
/// 残留的空目录也算进来，而那些目录写进去没有任何效果（客户端不读）。
pub fn find_loomy_roots() -> Vec<LoomyRoot> {
    find_loomy_roots_with(super::config::AppConfig::load().loomy_path.as_deref())
}

/// 同上，但手动指定的路径由调用方给出。
///
/// # 为什么拆出这个
///
/// 测试要验证"手动指定优先"，但不能去改**进程全局**的配置
///（并行测试下会互相干扰，实测把垃圾写进了用户真实配置）。
/// 把"手动路径"作为参数传入，测试就能给出临时值而不碰任何全局状态。
pub fn find_loomy_roots_with(manual: Option<&str>) -> Vec<LoomyRoot> {
    let mut out = Vec::new();

    // ① 用户手动指定
    if let Some(spec) = manual {
        let spec = spec.trim();
        if !spec.is_empty() {
            if let Some(r) = super::config::resolve_root(spec) {
                out.push(r);
            }
        }
    }

    // ② 公共安装目录（Loomy 默认装在这里，按用户/安装实例分子目录）
    if let Some(public_base) = public_loomy_base() {
        if let Ok(entries) = std::fs::read_dir(&public_base) {
            for entry in entries.flatten() {
                let root = entry.path();
                if !root.is_dir() {
                    continue;
                }
                let auth_file = root.join("userData").join("auth-session.json");
                if auth_file.is_file() {
                    push_unique(&mut out, LoomyRoot { root, auth_file });
                }
            }
        }
    }

    // ③ 旧版/便携版可能落在 %APPDATA%\Loomy
    if let Some(appdata) = appdata_dir() {
        for name in ["Loomy", "loomy"] {
            let root = appdata.join(name);
            let auth_file = root.join("auth-session.json");
            if auth_file.is_file() {
                push_unique(&mut out, LoomyRoot { root, auth_file });
            }
        }
    }

    out
}

/// 按**规范化路径**去重后加入。
///
/// 手动指定的路径可能与自动探测的结果是同一个 —— 不去重会让界面显示
/// 两个"同一处安装"，用户以为有两个 Loomy。
fn push_unique(out: &mut Vec<LoomyRoot>, r: LoomyRoot) {
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let c = canon(&r.root);
    if out.iter().any(|x| canon(&x.root) == c) {
        return;
    }
    out.push(r);
}

/// `C:\Users\Public\Loomy`。
fn public_loomy_base() -> Option<PathBuf> {
    std::env::var_os("PUBLIC")
        .map(PathBuf::from)
        .map(|p| p.join("Loomy"))
        .or_else(|| Some(PathBuf::from("C:\\Users\\Public\\Loomy")))
        .filter(|p| p.exists())
}

/// `%APPDATA%`（Windows 的 Roaming）。
pub fn appdata_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// `%APPDATA%` 下 Loomy 的 localStorage leveldb 目录。
///
/// # 为什么返回**多个**
///
/// 本机实测同时存在 `%APPDATA%\Loomy\...` 与 `%APPDATA%\loomy\...`
/// （大小写不同）。Windows 文件系统不区分大小写，所以它们**通常**是
/// 同一个目录；但用不同大小写拼路径在两个分支里写，会让人以为
/// 有两个副本。这里去重（按规范化路径）后返回。
pub fn find_leveldb_dirs() -> Vec<PathBuf> {
    let Some(appdata) = appdata_dir() else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = Vec::new();
    for name in ["Loomy", "loomy"] {
        let dir = appdata.join(name).join("Local Storage").join("leveldb");
        if !dir.is_dir() {
            continue;
        }
        // 去重：Windows 上大小写不敏感，用 canonicalize 归一
        let canon = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if out
            .iter()
            .any(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == canon)
        {
            continue;
        }
        out.push(dir);
    }
    out
}

/// leveldb 目录是否真的属于 Loomy —— 看里面有没有 Loomy 的 localStorage key。
///
/// # 为什么不能只看目录名
///
/// 本机实测 `%APPDATA%` 下有 **33 个** 应用都带 `Local Storage\leveldb` 结构：
///
/// ```text
/// Loomy / Kiro / Cursor / Code / QQ / BaiduNetdisk / bilibili / ... 都是这个结构
/// ```
///
/// 名字匹配到多个时（例如用户装过 `Loomy` 又装过 `LoomyXxx`），靠内容才能
/// 确定哪个是我们要的。
///
/// # 局限（实测过，写清楚免得误以为它是万能的）
///
/// - Loomy **运行中**时日志文件被独占锁住，读不了 → 返回 `false`
///   （所以调用方**不能**把它当唯一的判据，只能当"名字有歧义时的辅助"）
/// - 日志被 LevelDB 压缩进 `.ldb` 后是二进制，明文搜不到 key
///
/// 所以它只用于"名字命中多个、需要消歧"的场景，且读不到时应当**保守放行**
///（宁可用名字匹配的结论，也不要因为读不了就把目标排除掉）。
pub fn leveldb_looks_like_loomy(dir: &Path) -> bool {
    const KEY: &[u8] = b"loomy-auth-session";
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // 只看 .log（明文可读）；.ldb 是压缩格式，搜不到
        if !name.ends_with(".log") {
            continue;
        }
        // 用只读共享模式打开 —— Loomy 运行时独占锁会让 File::open 失败，
        // 但即便如此也只是"读不到"，不该 panic
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        // key 在日志里是明文（前面带 varint 长度前缀），直接窗口搜索
        if bytes.windows(KEY.len()).any(|w| w == KEY) {
            return true;
        }
    }
    false
}

/// 找出 Loomy 客户端主程序的位置。
///
/// # 为什么不能硬编码
///
/// 第一版把 `D:\software\Loomy-Setup-0.9.37\Loomy.exe` 写进了代码 ——
/// 那是我开发机的路径。分发出去后别人的安装位置不同，就会"找不到 Loomy.exe"，
/// 表现为"切换并重启 Loomy"静默失败。
///
/// 所以按可靠性从高到低依次尝试：
///
/// ```text
/// ① 注册表 Uninstall 键的 DisplayIcon   ← 安装程序写的，最权威
/// ② 正在运行的 Loomy 进程的 exe 路径     ← 它就在跑，位置一定对
/// ③ 常见安装目录下的 Loomy.exe           ← 兜底
/// ```
pub fn find_loomy_exe() -> Option<PathBuf> {
    // ① 注册表
    if let Some(p) = exe_from_registry() {
        return Some(p);
    }

    // ② 运行中的进程（任务管理器里的 Image Path）
    if let Some(p) = exe_from_running_process() {
        return Some(p);
    }

    // ③ 常见位置
    let mut cands: Vec<PathBuf> = vec![
        PathBuf::from(r"C:\Program Files\Loomy\Loomy.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Loomy\Loomy.exe"),
    ];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        cands.push(
            Path::new(&local)
                .join("Programs")
                .join("Loomy")
                .join("Loomy.exe"),
        );
        cands.push(Path::new(&local).join("Loomy").join("Loomy.exe"));
    }
    // 从已知的安装根反推：<安装根>\..\Loomy.exe（绿色版常把 exe 放同级）
    for r in find_loomy_roots() {
        if let Some(parent) = r.root.parent() {
            cands.push(parent.join("Loomy.exe"));
            // 也看看父目录下的子目录（D:\software\Loomy-Setup-x.y.z\Loomy.exe）
            if let Ok(entries) = std::fs::read_dir(parent) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        cands.push(p.join("Loomy.exe"));
                    }
                }
            }
        }
    }

    cands.into_iter().find(|p| p.is_file())
}

/// 从注册表 Uninstall 键读取 Loomy.exe 路径。
#[cfg(windows)]
fn exe_from_registry() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    // 64 位与 32 位视图都要看（Loomy 的位数不确定）
    let roots: [(RegKey, &str); 4] = [
        (RegKey::predef(HKEY_LOCAL_MACHINE), UNINSTALL),
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (RegKey::predef(HKEY_CURRENT_USER), UNINSTALL),
        (
            RegKey::predef(HKEY_CURRENT_USER),
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];

    for (root, path) in roots {
        let Ok(key) = root.open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for sub in key.enum_keys().flatten() {
            let Ok(app) = key.open_subkey_with_flags(&sub, KEY_READ) else {
                continue;
            };
            let display_name: String = app.get_value("DisplayName").unwrap_or_default();
            if !display_name.to_ascii_lowercase().contains("loomy") {
                continue;
            }
            let icon: String = app.get_value("DisplayIcon").unwrap_or_default();
            if icon.is_empty() {
                continue;
            }
            // DisplayIcon 常见形态：`D:\path\Loomy.exe,0`（末尾带图标索引）
            let raw = icon
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"');
            let p = PathBuf::from(raw);
            if p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("Loomy.exe"))
            {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn exe_from_registry() -> Option<PathBuf> {
    None
}

/// 从正在运行的 Loomy 进程拿 exe 路径。
#[cfg(windows)]
fn exe_from_running_process() -> Option<PathBuf> {
    // 用 wmic 会慢且有兼容问题；直接读进程模块路径更直接。
    // 这里用一个轻量办法：tasklist 不给路径，所以改用 PowerShell 的
    // Get-Process 太重。折中：用 std::process 调 `wmic process`。
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let out = std::process::Command::new("wmic")
        .args([
            "process",
            "where",
            "name='Loomy.exe'",
            "get",
            "ExecutablePath",
            "/value",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let line = line.trim();
        // 输出形如 `ExecutablePath=D:\software\...\Loomy.exe`
        if let Some(v) = line.strip_prefix("ExecutablePath=") {
            let p = PathBuf::from(v.trim());
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn exe_from_running_process() -> Option<PathBuf> {
    None
}

/// leveldb 目录里的**当前**日志文件（写入目标）。
///
/// LevelDB 把日志写成 `<编号>.log`，编号递增。取编号最大的那个 ——
/// 旧编号是已封存的段，往里写新记录客户端不会读。
pub fn current_leveldb_log(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(stem) = name.strip_suffix(".log") else {
            continue;
        };
        let Ok(num) = stem.parse::<u64>() else {
            continue;
        };
        if best.as_ref().is_none_or(|(n, _)| num > *n) {
            best = Some((num, entry.path()));
        }
    }
    best.map(|(_, p)| p)
}

/// 某安装根下所有 `opencode.json`（跳过 skills/templates/cache/node_modules）。
///
/// 与 Loomy 客户端自己的查找规则一致（`electron/auth/auth-session-controller`
/// 通过 opencodeService 同步，而 opencode 会扫这些位置）。
pub fn find_opencode_files(root: &Path) -> Vec<PathBuf> {
    const SKIP: &[&str] = &["skills", "templates", "cache", "node_modules"];
    let mut out = Vec::new();
    collect_opencode(root, SKIP, &mut out);
    out
}

fn collect_opencode(dir: &Path, skip: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if skip.iter().any(|s| name == *s) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_opencode(&path, skip, out);
        } else if name == "opencode.json" {
            out.push(path);
        }
    }
}

/// 本工具自己的数据目录（账号库、备份）。
///
/// # 为什么用 `%APPDATA%` 而不是 exe 旁边
///
/// 第一版放在 exe 所在目录的 `data/` 下 —— 那样"整个目录拷走就带走账号"，
/// 看起来便携。但它有个**装上去就坏**的问题：
///
/// ```text
/// 开发    .../target/debug/loomy-account-manager.exe   ← 可写
/// MSI 装到 C:\Program Files\LoomyAccountManager\       ← **普通用户不可写**
/// ```
///
/// 写不进去的后果是"导入账号报权限错误"，而用户完全不知道为什么
///（看起来像程序 bug，其实是 UAC）。
///
/// `%APPDATA%\<标识>\` 是 Windows 上放用户数据的标准位置：
/// 一定可写、随用户漫游、卸载时能一并清理。
///
/// 开发时（debug 构建）仍用 `%APPDATA%`，这样**测试与真实运行看到
/// 同一份数据** —— 否则开发时导入的账号在发布版里看不到，
/// 那种"数据去哪了"的困惑不值得省这点路径区分。
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("LoomyAccountManager"))
        .unwrap_or_else(|| PathBuf::from(".").join("loomy-data"));

    // 目录不存在时创建（写账号库/备份前都会经过这里）
    let _ = std::fs::create_dir_all(&base);
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 注册表里应当能找到 Loomy（本机实测有 `DisplayName = Loomy 0.9.38`）。
    ///
    /// 找不到就跳过 —— 有些机器可能没写注册表项。
    #[test]
    fn finds_exe_from_registry_or_common_paths() {
        if let Some(p) = exe_from_registry() {
            println!("注册表给出: {}", p.display());
            assert!(p.is_file(), "注册表给出的路径必须真实存在");
        } else {
            eprintln!("跳过：注册表里没有 Loomy");
        }
    }

    /// `find_loomy_exe` 绝不能返回硬编码的、别的机器上的路径。
    ///
    /// 这条钉住第一版的 bug：写死的 `D:\software\Loomy-Setup-0.9.37\Loomy.exe`
    /// 在开发机上有、在别人机器上没有，会导致"重启 Loomy"失败。
    #[test]
    fn exe_result_always_exists_on_this_machine() {
        if let Some(p) = find_loomy_exe() {
            assert!(
                p.is_file(),
                "返回的 exe 路径必须在本机真实存在（否则就是硬编码了别的机器）: {}",
                p.display()
            );
            assert!(
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().contains("loomy")),
                "返回的文件名应当含 loomy: {}",
                p.display()
            );
        }
    }

    /// 内容校验：本机 Loomy 的 leveldb 里应当能找到 key（Loomy 未运行时）。
    #[test]
    fn content_check_recognizes_real_loomy_leveldb() {
        let dirs = find_leveldb_dirs();
        let Some(dir) = dirs.first() else {
            eprintln!("跳过：本机没有 Loomy leveldb");
            return;
        };
        // Loomy 运行中时日志被独占锁住，读不到 key —— 这是已知局限，跳过
        if !crate::session::loomy_pids().is_empty() {
            eprintln!("跳过：Loomy 正在运行，日志被锁住读不了");
            return;
        }
        assert!(
            leveldb_looks_like_loomy(dir),
            "本机 Loomy 的 leveldb 里应当能找到 loomy-auth-session: {}",
            dir.display()
        );
    }

    /// 无关应用的 leveldb 不该被误认成 Loomy。
    ///
    /// 本机 %APPDATA% 下有 33 个应用都是同样的目录结构 —— 这条确保
    /// 内容校验真的能区分它们,而不是"只要有 .log 就返回 true"。
    #[test]
    fn content_check_rejects_other_apps() {
        let Some(appdata) = appdata_dir() else {
            return;
        };
        let mut checked = 0;
        for name in ["Kiro", "Cursor", "Code", "QQ", "doubao-account-manager"] {
            let dir = appdata.join(name).join("Local Storage").join("leveldb");
            if !dir.is_dir() {
                continue;
            }
            checked += 1;
            assert!(
                !leveldb_looks_like_loomy(&dir),
                "{name} 的 leveldb 不该被认成 Loomy: {}",
                dir.display()
            );
        }
        if checked == 0 {
            eprintln!("跳过：本机没有其它 Electron 应用的 leveldb 可比对");
        } else {
            println!("已确认 {checked} 个其它应用未被误认");
        }
    }

    /// 手动指定的路径必须排在自动探测结果**之前**（用户明确指定的优先）。
    #[test]
    fn manual_override_takes_priority() {
        // 用参数传入手动路径，**完全不碰全局状态**。
        //
        // 走过两条弯路，都记在这里避免重犯：
        //  ① 直接写真实配置再"还原" —— 还原的是"测试开始时的快照"，
        //     若那条快照已被别的测试改过，就把垃圾留给了用户
        //  ② 改用 `LOOMY_MGR_CONFIG_DIR` 环境变量 —— 但 cargo 默认并行跑测试，
        //     环境变量是进程全局的，两条测试 set/remove 互相干扰，照样污染
        // 结论：测试要隔离，就**不能依赖进程全局状态**，得把依赖参数化。
        let base = std::env::temp_dir().join(format!("prio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);

        let root = base.join("manual-root");
        std::fs::create_dir_all(root.join("userData")).unwrap();
        std::fs::write(root.join("userData").join("auth-session.json"), "{}").unwrap();

        let found = find_loomy_roots_with(Some(&root.display().to_string()));
        assert!(!found.is_empty(), "应当至少有手动指定的那个");
        assert_eq!(
            found[0].root, root,
            "手动指定的必须排在第一个（用户指定的优先于我们猜的）"
        );

        // 不带手动路径时，这个临时目录不该出现在结果里
        let auto = find_loomy_roots_with(None);
        assert!(
            !auto.iter().any(|r| r.root == root),
            "临时目录不该被自动探测扫到"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// 手动路径无效（不存在/指向无关目录）时，应当**静默退回**自动探测，
    /// 而不是让整个列表为空 —— 否则用户配错一次就再也用不了了。
    #[test]
    fn invalid_manual_path_falls_back_to_auto() {
        let found = find_loomy_roots_with(Some(r"Z:\definitely\not\here"));
        let auto = find_loomy_roots_with(None);
        assert_eq!(found.len(), auto.len(), "无效的手动路径应当等价于没指定");
    }
}
