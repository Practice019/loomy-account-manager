//! 账号元数据命令：余额、新手任务、邀请码。

use crate::clients::{http, points};
use crate::core::account;
use crate::core::error::{Error, Result};

/// 取某账号的 session（命令层与 points 模块之间的唯一桥）。
fn session_of(userid: &str) -> Result<String> {
    let list = account::load()?;
    list.iter()
        .find(|a| a.userid == userid)
        .map(|a| a.session.clone())
        .ok_or_else(|| Error::NotFound(format!("账号不在库里: {userid}")))
}

/// 查余额。
#[tauri::command]
pub async fn fetch_balance(userid: String) -> Result<points::Balance> {
    let s = session_of(&userid)?;
    points::balance(&http::shared(), &s).await
}

/// 查新手任务进度。
#[tauri::command]
pub async fn fetch_tasks(userid: String) -> Result<points::TaskProgress> {
    let s = session_of(&userid)?;
    points::tasks(&http::shared(), &s).await
}

/// 完成**全部**未完成的新手任务，返回逐项结果。
///
/// # 为什么逐项返回而不是只回一个总数
///
/// 有的任务上游会拒（例如需要客户端交互的），只回总数会让用户
/// 以为"点了没反应"。逐项列出"哪几个成了、哪几个失败为什么"才有用。
///
/// # 为什么串行而不是并发
///
/// 这些接口共用一个 session，并发打可能触发上游限流，
/// 而总分只有一万——串行的几秒钟完全可接受。
#[tauri::command]
pub async fn complete_all_tasks(userid: String) -> Result<Vec<TaskResult>> {
    let s = session_of(&userid)?;
    let client = http::shared();

    let progress = points::tasks(&client, &s).await?;
    let mut out = Vec::new();
    for t in progress.tasks.iter().filter(|t| !t.done) {
        let r = match points::complete_task(&client, &s, &t.key).await {
            Ok((true, got)) => TaskResult {
                key: t.key.clone(),
                label: t.label.clone(),
                ok: true,
                points: got,
                error: String::new(),
            },
            Ok((false, _)) => TaskResult {
                key: t.key.clone(),
                label: t.label.clone(),
                ok: false,
                points: 0,
                error: "上游未确认完成".into(),
            },
            Err(e) => TaskResult {
                key: t.key.clone(),
                label: t.label.clone(),
                ok: false,
                points: 0,
                error: e.to_string(),
            },
        };
        out.push(r);
    }
    Ok(out)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResult {
    pub key: String,
    pub label: String,
    pub ok: bool,
    pub points: i64,
    pub error: String,
}

/// 查激活状态（用了谁的邀请码）。
#[tauri::command]
pub async fn fetch_activation(userid: String) -> Result<points::ActivationState> {
    let s = session_of(&userid)?;
    points::activation(&http::shared(), &s).await
}

/// 绑定邀请码。
#[tauri::command]
pub async fn bind_invite(userid: String, code: String) -> Result<()> {
    let s = session_of(&userid)?;
    points::bind_invite(&http::shared(), &s, &code).await
}

/// 查自己生成的邀请码。
///
/// # 空列表时自动补一次 `first-login`
///
/// 客户端**每次登录后**都会自动调 `POST /points/first-login` 完成积分账号初始化，
/// 服务端借此下发注册奖励并**生成 5 个邀请码**。而通过本工具导入的账号没有
/// 走过这一步，于是 `invitation-codes` 返回**空列表** —— 现象是"我没有邀请码"，
/// 而排在账号列表里的其它号都有。
///
/// 所以这里：查询成功但结果为空 → 自动补一次首登 → 再查一次。
/// 服务端有 `alreadyProcessed` 幂等保护，重复调用不会重复发奖。
///
/// 注意只在**查询成功且为空**时才补：查询本身失败（网络/鉴权）时补首登没有意义，
/// 反而会把真正的错误盖掉。
#[tauri::command]
pub async fn fetch_my_invites(userid: String) -> Result<Vec<points::InviteCode>> {
    let s = session_of(&userid)?;
    let client = http::shared();

    let codes = points::my_invite_codes(&client, &s).await?;
    if !codes.is_empty() {
        return Ok(codes);
    }

    // 补初始化；失败就返回原来的空列表（这是"附加信息"，不该让整个调用报错）。
    points::first_login(&client, &s).await?;
    points::my_invite_codes(&client, &s).await
}

/// 首登奖励。
#[tauri::command]
pub async fn claim_first_login(userid: String) -> Result<()> {
    let s = session_of(&userid)?;
    points::first_login(&http::shared(), &s).await
}

/// 批量查余额（给账号列表用，避免 N+1 次 IPC 往返）。
///
/// 单次网络失败**不**让整个批量失败 —— 那一个账号显示"查询失败"即可，
/// 其余照常。
#[tauri::command]
pub async fn fetch_balances(userids: Vec<String>) -> Result<Vec<BalanceRow>> {
    let client = http::shared();
    let list = account::load()?;
    let mut out = Vec::new();
    for uid in userids {
        let Some(a) = list.iter().find(|a| a.userid == uid) else {
            out.push(BalanceRow {
                userid: uid,
                available: 0,
                daily: 0,
                balance: 0,
                error: "账号不在库里".into(),
            });
            continue;
        };
        match points::balance(&client, &a.session).await {
            Ok(b) => out.push(BalanceRow {
                userid: uid,
                available: b.available_balance,
                daily: b.daily_balance,
                balance: b.balance,
                error: String::new(),
            }),
            Err(e) => out.push(BalanceRow {
                userid: uid,
                available: 0,
                daily: 0,
                balance: 0,
                error: e.to_string(),
            }),
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceRow {
    pub userid: String,
    pub available: i64,
    pub daily: i64,
    pub balance: i64,
    pub error: String,
}
