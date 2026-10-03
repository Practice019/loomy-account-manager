//! 写登录态到三个位置。
//!
//! # 三个位置缺一不可（这是本工具的核心知识）
//!
//! ```text
//! ① auth-session.json        electron-store，主进程启动时读
//! ② opencode.json            provider.imodel.options.apiKey
//! ③ localStorage (leveldb)   渲染进程读它 **并覆盖 ①**
//! ```
//!
//! 只写 ①② 时切换**不生效** —— 下次启动 Loomy 会用 ③ 里的旧值把 ① 覆盖回去。
//! 这个坑实测踩过（时间线见 `session/leveldb.rs` 的模块注释）。
//!
//! 反过来，只写 ③ 也不行：主进程启动时读的是 ①。
//! 所以必须**三处一起写**，而且每一处失败都要报出来（不能静默）。

pub mod helpers;
pub mod leveldb;

pub use helpers::{paths_active_session, read_current_session_from_default};

use std::path::Path;

use crate::core::error::{Error, Result};
use crate::core::paths::{self, LoomyRoot};

/// 写入结果（逐处报告，便于 UI 显示"哪处没写成功"）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    /// ① 是否写了 auth-session.json
    pub json_written: bool,
    /// ② 写了几个 opencode.json
    pub opencode_files: Vec<String>,
    /// ③ 写了几个 leveldb 日志
    pub leveldb_logs: Vec<String>,
    /// 非致命问题的说明（例如 leveldb 目录找不到）
    pub warnings: Vec<String>,
}

impl ApplyReport {
    /// 三处是否都写到了。
    ///
    /// ⚠ `opencode.json` 不计入：`useSessionAuth=true` 时那边 apiKey
    /// 本来就可以为空（运行时由主进程注入），写空值是正常状态。
    pub fn fully_applied(&self) -> bool {
        self.json_written && !self.leveldb_logs.is_empty()
    }
}

/// 把某个账号的 session 应用到本机 Loomy（三处一起写）。
pub fn apply(root: &LoomyRoot, account: &crate::core::account::Account) -> Result<ApplyReport> {
    let mut report = ApplyReport {
        json_written: false,
        opencode_files: Vec::new(),
        leveldb_logs: Vec::new(),
        warnings: Vec::new(),
    };

    // ── ① auth-session.json ───────────────────────────────────────────────
    //
    // 形状必须与 Loomy 的 `AUTH_SESSION_DEFAULTS` 完全一致
    //（session/userid/phone/updatedAt），多字段少字段都可能被它判成无效。
    let payload = serde_json::json!({
        "session": account.session,
        "userid": account.userid,
        "phone": account.phone,
        "updatedAt": chrono::Utc::now().timestamp_millis(),
    });
    std::fs::write(
        &root.auth_file,
        serde_json::to_string_pretty(&payload)? + "\n",
    )?;
    report.json_written = true;

    // ── ② opencode.json → imodel.apiKey ───────────────────────────────────
    for f in paths::find_opencode_files(&root.root) {
        match patch_opencode(&f, &account.session) {
            Ok(true) => report.opencode_files.push(
                f.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            Ok(false) => {} // 没有 provider.imodel，不是这个文件的事
            Err(e) => report
                .warnings
                .push(format!("写 {} 失败: {e}", f.display())),
        }
    }

    // ── ③ localStorage (leveldb) ──────────────────────────────────────────
    //
    // ⚠ 这一处是决定性的。失败必须让用户知道 —— 否则现象是
    // "切换看着成功了，重启 Loomy 又变回旧账号"。
    let stored = leveldb::StoredSession {
        phone: account.phone.clone(),
        masked_phone: leveldb::mask_phone(&account.phone),
        session: account.session.clone(),
        userid: account.userid.clone(),
        logged_in_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    };
    match leveldb::write_session_all(&stored) {
        Ok(logs) => report.leveldb_logs = logs.iter().map(|p| p.display().to_string()).collect(),
        Err(e) => report.warnings.push(format!(
            "localStorage 写入失败：{e}\n\
             ⚠ 这会导致切换**不生效**（Loomy 启动时会用 localStorage 里的旧账号覆盖 auth-session.json）"
        )),
    }

    Ok(report)
}

/// 更新一个 `opencode.json` 的 `provider.imodel.options.apiKey`。
///
/// 返回 `Ok(false)` 表示这个文件里没有 `provider.imodel.options`
/// （不是它的职责，跳过而非报错）。
fn patch_opencode(path: &Path, session: &str) -> Result<bool> {
    let raw = std::fs::read_to_string(path)?;
    let mut cfg: serde_json::Value = serde_json::from_str(&raw)?;

    let Some(opts) = cfg
        .get_mut("provider")
        .and_then(|p| p.get_mut("imodel"))
        .and_then(|m| m.get_mut("options"))
    else {
        return Ok(false);
    };

    opts["apiKey"] = serde_json::Value::String(session.to_string());
    // 与 Loomy 自身一致：apiKey 就是 session
    if opts.get("useSessionAuth").is_none() {
        opts["useSessionAuth"] = serde_json::Value::Bool(true);
    }

    std::fs::write(path, serde_json::to_string_pretty(&cfg)? + "\n")?;
    Ok(true)
}

/// 读本机当前激活的账号（来自 auth-session.json）。
pub fn read_active(root: &LoomyRoot) -> Option<crate::core::account::Account> {
    let raw = std::fs::read_to_string(&root.auth_file).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let session = v.get("session")?.as_str()?.trim().to_string();
    if session.is_empty() {
        return None;
    }
    Some(crate::core::account::Account {
        userid: v
            .get("userid")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        session,
        phone: v
            .get("phone")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        nickname: String::new(),
        name: String::new(),
        note: String::new(),
        source: "active".into(),
        imported_at: 0,
    })
}

/// 检查 Loomy 是否在运行，返回进程 ID 列表。
///
/// # 为什么要返回值而不是布尔
///
/// 提示里带上 PID 能让用户确认"关对了进程"，也让"关了但没关干净"
/// 这种情况可排查。
pub fn loomy_pids() -> Vec<u32> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Loomy.exe", "/NH", "/FO", "CSV"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        let Ok(out) = out else { return Vec::new() };
        let text = String::from_utf8_lossy(&out.stdout);
        let mut pids = Vec::new();
        for line in text.lines() {
            // CSV: "Loomy.exe","1234","Console","1","123,456 K"
            let cols: Vec<&str> = line.split("\",\"").collect();
            if cols.len() >= 2 {
                let pid = cols[1].trim_matches('"');
                if let Ok(n) = pid.parse::<u32>() {
                    pids.push(n);
                }
            }
        }
        pids
    }
    #[cfg(not(windows))]
    {
        // 非 Windows 用 pgrep（Loomy 目前主要在 Windows，这里是兜底）
        let out = std::process::Command::new("pgrep")
            .arg("-f")
            .arg("Loomy")
            .output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.trim().parse::<u32>().ok())
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

/// 退出 Loomy（切换前必须）。
///
/// `force=true` 时用 `/F` 强杀 —— Loomy 有时会有残留进程不响应正常关闭，
/// 而那些进程仍占着 leveldb 文件锁。
#[cfg(windows)]
pub fn kill_loomy(force: bool) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut args = vec!["/IM", "Loomy.exe"];
    if force {
        args.push("/F");
    }
    let out = std::process::Command::new("taskkill")
        .args(&args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        // "没有运行的任务"不算错误
        if !msg.contains("not found") && !msg.contains("没有") {
            return Err(Error::Other(format!("结束 Loomy 失败: {}", msg.trim())));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn kill_loomy(_force: bool) -> Result<()> {
    let out = std::process::Command::new("pkill")
        .arg("-f")
        .arg("Loomy")
        .output()?;
    if !out.status.success() {
        return Err(Error::Other("结束 Loomy 失败（pkill）".into()));
    }
    Ok(())
}

/// 等 Loomy 真的退出（最多 `timeout_ms`）。
///
/// # 为什么要等
///
/// `taskkill` 返回时进程可能还在退出中（要把内存里的 localStorage
/// 刷回磁盘）。这时我们写 leveldb 就会被它的收尾写入覆盖 ——
/// 所以必须**确认进程真的没了**再写。
pub fn wait_loomy_exit(timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < timeout_ms as u128 {
        if loomy_pids().is_empty() {
            // 再等一小会儿，让文件句柄彻底释放
            std::thread::sleep(std::time::Duration::from_millis(500));
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    loomy_pids().is_empty()
}
