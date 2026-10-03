//! 账号相关命令：列表、导入、导出、删除、改名。

use crate::core::account::{self, Account};
use crate::core::error::{Error, Result};
use crate::session;

/// 一个账号 + 它的运行时状态（给 UI 一次性显示全）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    #[serde(flatten)]
    pub account: Account,
    /// 是否是当前激活的账号
    pub active: bool,
    /// 展示名（UI 直接用，免得前端再算一遍规则）
    pub display_name: String,
}

/// 列出全部账号（含"哪个是当前激活的"）。
#[tauri::command]
pub fn list_accounts() -> Result<Vec<AccountView>> {
    let list = account::load()?;
    let active = session::paths_active_session();
    Ok(list
        .into_iter()
        .map(|a| AccountView {
            active: active.as_deref() == Some(a.session.as_str()),
            display_name: a.display_name(),
            account: a,
        })
        .collect())
}

/// 当前本机 Loomy 登录的账号（可能不在托管库里）。
#[tauri::command]
pub fn active_account() -> Result<Option<Account>> {
    let Some(root) = crate::core::paths::find_loomy_roots().into_iter().next() else {
        return Ok(None);
    };
    Ok(session::read_active(&root))
}

/// 导入结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: usize,
    pub updated: usize,
    /// 无法解析的条目及原因 —— **必须回给用户**，静默跳过是"我明明导了却没了"的成因
    pub skipped: Vec<String>,
    /// 导入后库里共几个
    pub total: usize,
}

/// 从粘贴的文本导入（JSON / JSON 数组 / 多个 JSON 混在一起）。
#[tauri::command]
pub fn import_text(text: String) -> Result<ImportReport> {
    let parsed = account::parse_text(&text)?;
    let mut list = account::load()?;
    let mut report = ImportReport {
        added: 0,
        updated: 0,
        skipped: Vec::new(),
        total: 0,
    };
    for mut a in parsed {
        a.source = if a.source.is_empty() {
            "paste".into()
        } else {
            a.source
        };
        if account::upsert(&mut list, a) {
            report.added += 1;
        } else {
            report.updated += 1;
        }
    }
    account::save(&list)?;
    report.total = list.len();
    Ok(report)
}

/// 从目录批量导入（支持 `auths/<上游>/*.json` 与 `*.json` 两种布局）。
#[tauri::command]
pub fn import_dir(dir: String) -> Result<ImportReport> {
    let p = std::path::PathBuf::from(&dir);
    if !p.is_dir() {
        return Err(Error::NotFound(format!("目录不存在: {dir}")));
    }
    let (found, skipped) = account::import_from_dir(&p);
    if found.is_empty() {
        return Err(Error::Invalid(format!(
            "在 {dir} 里没找到任何账号。\n\n\
             找过这些位置：\n\
             - {dir} 下的 *.json\n\
             - {dir}/auths 及它下一层子目录里的 *.json\n\n\
             跳过 {} 个文件{}",
            skipped.len(),
            if skipped.is_empty() {
                String::new()
            } else {
                format!("：\n{}", skipped.join("\n"))
            }
        )));
    }

    let mut list = account::load()?;
    let mut report = ImportReport {
        added: 0,
        updated: 0,
        skipped,
        total: 0,
    };
    for a in found {
        if account::upsert(&mut list, a) {
            report.added += 1;
        } else {
            report.updated += 1;
        }
    }
    account::save(&list)?;
    report.total = list.len();
    Ok(report)
}

/// 删一个账号。
#[tauri::command]
pub fn delete_account(userid: String) -> Result<usize> {
    let mut list = account::load()?;
    let before = list.len();
    list.retain(|a| a.userid != userid);
    if list.len() == before {
        return Err(Error::NotFound(format!("账号不存在: {userid}")));
    }
    account::save(&list)?;
    Ok(list.len())
}

/// 改备注 / 别名（用户自己标"主号"/"备用"用）。
#[tauri::command]
pub fn update_account_note(userid: String, note: String, name: Option<String>) -> Result<()> {
    let mut list = account::load()?;
    let a = list
        .iter_mut()
        .find(|a| a.userid == userid)
        .ok_or_else(|| Error::NotFound(format!("账号不存在: {userid}")))?;
    a.note = note;
    if let Some(n) = name {
        a.name = n;
    }
    account::save(&list)
}

/// 导出一个账号的 token（给别处用）。
#[tauri::command]
pub fn export_account(userid: String) -> Result<String> {
    let list = account::load()?;
    let a = list
        .iter()
        .find(|a| a.userid == userid)
        .ok_or_else(|| Error::NotFound(format!("账号不存在: {userid}")))?;
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "session": a.session,
        "userid": a.userid,
        "phone": a.phone,
        "nickname": a.nickname,
    }))?)
}
