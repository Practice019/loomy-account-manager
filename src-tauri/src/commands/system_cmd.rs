//! 系统状态命令 —— 真实返回本机 Loomy 的位置，供设置页显示。
//!
//! # 为什么需要这个命令（它修的是一个真 bug）
//!
//! 设置页原来这样显示"数据目录"：
//!
//! ```ts
//! listBackups().then(() => setDataDir("%APPDATA%\\LoomyAccountManager"))
//! ```
//!
//! 那个字符串是**前端写死的字面量**，跟后端毫无关系 —— 如果实际数据目录
//! 不是这个（换了环境变量、或走了 `unwrap_or_else` 的兜底分支），界面会
//! 显示一个错误的位置，用户照着去找会一无所获。
//!
//! 现在改为调本命令，由后端报告**它自己实际用的**路径。

use crate::core::config::AppConfig;
use crate::core::paths;
use crate::session;

/// 本机 Loomy 环境的真实状态。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    /// 是否探测到了可用的 Loomy 登录态。
    pub found: bool,
    /// 安装根（可能多个 —— 多用户/多实例机器上会出现）。
    pub roots: Vec<String>,
    /// 实际使用的安装根（取第一个）。
    pub root: Option<String>,
    /// `auth-session.json` 路径。
    pub auth_file: Option<String>,
    /// leveldb 目录（多个时全部列出）。
    pub leveldb_dirs: Vec<String>,
    /// 当前要写入的 leveldb 日志。
    pub leveldb_log: Option<String>,
    /// Loomy.exe 路径。
    pub exe: Option<String>,
    /// 运行中的 PID。
    pub running_pids: Vec<u32>,
    /// 本工具自己的数据目录（**真实值**，不是前端猜的）。
    pub data_dir: String,
    /// 账号库文件路径。
    pub accounts_file: String,
    /// 用户手动指定的路径（None = 用自动探测）。
    pub manual_path: Option<String>,
    /// 自动探测失败时给出的候选（供 UI 预填，**不自动采用**）。
    pub guess_candidates: Vec<String>,
    /// 需要用户手动指定吗（探测失败 或 探测到多个）。
    pub needs_manual: bool,
    /// 为什么需要手动指定（给用户看的一句话）。
    pub reason: String,
}

/// 采集本机状态。
#[tauri::command]
pub fn system_status() -> SystemStatus {
    let roots = paths::find_loomy_roots();
    let leveldb_dirs = paths::find_leveldb_dirs();
    let cfg = AppConfig::load();

    let root = roots.first();
    let leveldb_log = leveldb_dirs
        .first()
        .and_then(|d| paths::current_leveldb_log(d));

    // 什么时候需要用户介入：
    //   - 一个都没找到 → 必须手动指定（否则整个工具没法用）
    //   - 找到多个 → 让用户选（静默取第一个在多实例机器上可能选错）
    let (needs_manual, reason) = if roots.is_empty() {
        (
            true,
            "没能自动找到 Loomy 的登录态。\
             可能是装在了非默认位置，或者 Loomy 还没运行过（登录态文件尚未生成）。\
             请手动指定 Loomy 的安装目录。"
                .to_string(),
        )
    } else if roots.len() > 1 {
        (
            true,
            format!(
                "找到 {} 个 Loomy 安装，无法确定用哪个。请手动指定一个。",
                roots.len()
            ),
        )
    } else if leveldb_dirs.is_empty() {
        // 这种情况不阻塞使用，但切换会不生效 —— 必须警告
        (
            false,
            "找到了登录态，但没找到 localStorage（leveldb）。\
             切换账号可能不生效 —— 请确认 Loomy 至少运行过一次。"
                .to_string(),
        )
    } else {
        (false, String::new())
    };

    SystemStatus {
        found: !roots.is_empty(),
        roots: roots.iter().map(|r| r.root.display().to_string()).collect(),
        root: root.map(|r| r.root.display().to_string()),
        auth_file: root.map(|r| r.auth_file.display().to_string()),
        leveldb_dirs: leveldb_dirs
            .iter()
            .map(|d| d.display().to_string())
            .collect(),
        leveldb_log: leveldb_log.map(|p| p.display().to_string()),
        exe: paths::find_loomy_exe().map(|p| p.display().to_string()),
        running_pids: session::loomy_pids(),
        // 真实值，不是前端写死的字符串
        data_dir: paths::data_dir().display().to_string(),
        accounts_file: crate::core::account::store_file().display().to_string(),
        manual_path: cfg.loomy_path.clone(),
        guess_candidates: if roots.is_empty() {
            crate::core::config::guess_candidates()
        } else {
            Vec::new()
        },
        needs_manual,
        reason,
    }
}

/// 手动指定 Loomy 位置。
///
/// # 为什么保存前必须校验
///
/// 用户可能选错目录。若只存下路径、等到切换时才发现不对，用户会在
/// "点切换 → 失败 → 回来改"之间来回，而错在哪一步并不明显。
/// 这里当场解析，失败就明确告知 —— 而且**不保存**。
///
/// 返回解析出的安装根（供 UI 立刻回显确认）。
#[tauri::command]
pub fn set_loomy_path(path: String) -> crate::core::error::Result<String> {
    use crate::core::error::Error;

    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(Error::Invalid("路径为空".into()));
    }

    let Some(root) = crate::core::config::resolve_root(trimmed) else {
        // 报错里说清"要选到哪一层"，因为多选/少选一层是最常见的错法
        return Err(Error::Invalid(format!(
            "在「{trimmed}」里没找到 Loomy 的登录态。\n\n\
             请选择包含 userData 的那一层目录，通常是：\n\
             C:\\Users\\Public\\Loomy\\<安装ID>\n\n\
             也可以直接选 userData 目录，或那个 auth-session.json 文件。"
        )));
    };

    let mut cfg = AppConfig::load();
    cfg.loomy_path = Some(trimmed.to_string());
    cfg.save()?;

    Ok(root.root.display().to_string())
}

/// 清掉手动指定，回到纯自动探测。
#[tauri::command]
pub fn clear_loomy_path() -> crate::core::error::Result<()> {
    AppConfig::clear_loomy_path()?;
    Ok(())
}

/// 校验一个路径（不保存）—— UI 上"测试这个路径"用。
#[tauri::command]
pub fn test_loomy_path(path: String) -> crate::core::error::Result<Option<String>> {
    Ok(crate::core::config::resolve_root(&path).map(|r| r.root.display().to_string()))
}
