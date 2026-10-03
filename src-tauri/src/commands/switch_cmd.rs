//! 切换账号相关命令。

use crate::core::account::{self, Account};
use crate::core::error::{Error, Result};
use crate::session;

/// 环境检查结果 —— 切换前先问一次，让 UI 能给出准确的引导。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchPrecheck {
    /// Loomy 安装根（找不到时为 null）
    pub root: Option<String>,
    /// 运行中的 Loomy 进程
    pub running_pids: Vec<u32>,
    /// 能否直接切换（Loomy 没在跑）
    pub can_switch_directly: bool,
    /// 当前激活账号的 userid
    pub active_userid: Option<String>,
}

/// 切换前的环境检查。
#[tauri::command]
pub fn precheck_switch() -> Result<SwitchPrecheck> {
    let roots = crate::core::paths::find_loomy_roots();
    let pids = session::loomy_pids();
    Ok(SwitchPrecheck {
        root: roots.first().map(|r| r.root.display().to_string()),
        running_pids: pids.clone(),
        can_switch_directly: pids.is_empty() && !roots.is_empty(),
        active_userid: roots
            .first()
            .and_then(session::read_active)
            .map(|a| a.userid),
    })
}

/// 切换结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchReport {
    pub userid: String,
    pub session: String,
    /// 备份目录名（可据此恢复）
    pub backup: String,
    /// 逐处写入的结果
    pub applied: session::ApplyReport,
    /// 是否顺手把 Loomy 重启了
    pub restarted: bool,
}

/// 切换到某个账号。
///
/// # 参数
///
/// - `userid`：库里的账号
/// - `kill_running`：若 Loomy 在运行，是否先关掉它（**必须**关才能生效）
/// - `restart_after`：写完是否重新启动 Loomy
///
/// # 为什么必须关掉 Loomy
///
/// 客户端运行中时它内存里有 localStorage 副本，退出时会刷回磁盘 ——
/// 我们的 leveldb 写入会被覆盖，于是"切换看着成功、重启又变回去"。
/// 这是客户端机制决定的，绕不过去，所以这里显式要求调用方确认。
#[tauri::command]
pub fn switch_account(
    userid: String,
    kill_running: bool,
    restart_after: bool,
) -> Result<SwitchReport> {
    let mut list = account::load()?;
    let acct: Account = list
        .iter()
        .find(|a| a.userid == userid)
        .cloned()
        .ok_or_else(|| Error::NotFound(format!("账号不在库里: {userid}（先导入）")))?;

    let roots = crate::core::paths::find_loomy_roots();
    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| Error::LoomyNotFound("找不到 userData/auth-session.json".into()))?;

    // ── 确保 Loomy 退出 ───────────────────────────────────────────────────
    let pids = session::loomy_pids();
    if !pids.is_empty() {
        if !kill_running {
            return Err(Error::LoomyRunning { pids });
        }
        session::kill_loomy(true)?;
        // 必须**等到真的退出**再写：taskkill 返回时它可能还在把内存里的
        // localStorage 刷回磁盘，那时我们写入会被它覆盖。
        if !session::wait_loomy_exit(15_000) {
            return Err(Error::Other(
                "Loomy 没有在 15 秒内退出，请手动结束后重试".into(),
            ));
        }
    }

    // ── 备份当前登录态（切换前）───────────────────────────────────────────
    let prev = session::read_active(&root);
    let prev_ldb = crate::session::read_current_session_from_default()
        .ok()
        .flatten();
    let name = crate::core::util::backup_name();
    let meta = account::BackupMeta {
        name: name.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
        userid: prev.as_ref().map(|a| a.userid.clone()).unwrap_or_default(),
        session: prev.as_ref().map(|a| a.session.clone()).unwrap_or_default(),
        leveldb: prev_ldb,
    };
    account::write_backup(&meta)?;

    // ── 写三处 ────────────────────────────────────────────────────────────
    let applied = session::apply(&root, &acct)?;

    // ── 可选重启 ──────────────────────────────────────────────────────────
    let mut restarted = false;
    if restart_after {
        if let Some(exe) = crate::core::paths::find_loomy_exe() {
            let _ = std::process::Command::new(exe).spawn();
            restarted = true;
        }
    }

    // 把"当前激活"记进库里（UI 据此显示勾）
    if let Some(a) = list.iter_mut().find(|a| a.userid == acct.userid) {
        a.imported_at = a.imported_at.max(1);
    }
    account::save(&list)?;

    Ok(SwitchReport {
        userid: acct.userid,
        session: acct.session,
        backup: name,
        applied,
        restarted,
    })
}

/// 恢复最近一次（或指定）备份。
#[tauri::command]
pub fn restore_backup(name: Option<String>) -> Result<SwitchReport> {
    let backups = account::list_backups();
    let chosen = match name {
        Some(n) => backups
            .into_iter()
            .find(|b| b.name == n)
            .ok_or_else(|| Error::NotFound(format!("备份不存在: {n}")))?,
        None => backups
            .into_iter()
            .next()
            .ok_or_else(|| Error::NotFound("没有任何备份".into()))?,
    };

    let roots = crate::core::paths::find_loomy_roots();
    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| Error::LoomyNotFound("找不到 Loomy 安装".into()))?;

    // 备份会话同样要先关 Loomy（同 switch 的理由）
    let pids = session::loomy_pids();
    if !pids.is_empty() {
        session::kill_loomy(true)?;
        if !session::wait_loomy_exit(15_000) {
            return Err(Error::Other("Loomy 没有在 15 秒内退出".into()));
        }
    }

    let acct = Account {
        userid: chosen.userid.clone(),
        session: chosen.session.clone(),
        phone: chosen
            .leveldb
            .as_ref()
            .map(|l| l.phone.clone())
            .unwrap_or_default(),
        nickname: String::new(),
        name: String::new(),
        note: String::new(),
        source: "restore".into(),
        imported_at: 0,
    };

    // 用 leveldb 备份里的 loggedInAt（保持时间线一致），而不是"现在"
    let applied = session::apply(&root, &acct)?;

    Ok(SwitchReport {
        userid: acct.userid,
        session: acct.session,
        backup: chosen.name,
        applied,
        restarted: false,
    })
}

/// 列备份。
#[tauri::command]
pub fn list_backups() -> Vec<account::BackupMeta> {
    account::list_backups()
}

/// 手动结束 Loomy（UI 上"关闭并切换"的按钮）。
#[tauri::command]
pub fn kill_loomy() -> Result<Vec<u32>> {
    session::kill_loomy(true)?;
    session::wait_loomy_exit(15_000);
    Ok(session::loomy_pids())
}

/// 启动 Loomy。
///
/// 用 `paths::find_loomy_exe()`（注册表 → 运行中进程 → 常见目录），
/// **不再硬编码路径** —— 第一版写死了开发机的 `D:\software\...`，
/// 换台机器就会"找不到 Loomy.exe"。
#[tauri::command]
pub fn launch_loomy() -> Result<bool> {
    let Some(exe) = crate::core::paths::find_loomy_exe() else {
        return Err(Error::NotFound(
            "找不到 Loomy.exe。请在「设置」里手动指定 Loomy 的位置，或手动启动它。".into(),
        ));
    };
    std::process::Command::new(exe).spawn()?;
    Ok(true)
}
