//! 账号模型与存储。
//!
//! # 存储形态
//!
//! 账号库是一个 JSON 数组，放在 `data/accounts.json`。
//! 写入用**原子替换**（写临时文件 → rename）—— 直接覆写的话，写到一半
//! 掉电/崩溃就会留下半个 JSON，**全部账号一起丢失**。账号是用户拿不到的
//! 东西（session 只能重新登录获取），所以这里必须稳。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::error::{Error, Result};
use crate::core::paths;

/// 一个托管账号。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub userid: String,
    pub session: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub name: String,
    /// 备注（用户自己写的，便于区分）
    #[serde(default)]
    pub note: String,
    /// 从哪儿import的（"paste" / "file" / "dir"，便于排查）
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub imported_at: i64,
}

impl Account {
    /// 展示用名称：备注 > 别名 > 昵称 > 手机号 > userid。
    pub fn display_name(&self) -> String {
        for s in [&self.note, &self.name, &self.nickname, &self.phone] {
            if !s.trim().is_empty() {
                return s.clone();
            }
        }
        self.userid.clone()
    }
}

/// 账号库文件路径。
pub fn store_file() -> PathBuf {
    paths::data_dir().join("accounts.json")
}

/// 读出全部账号。
///
/// 文件不存在 → 空列表（首次运行的正常状态，不是错误）。
/// 文件损坏 → **报错而不是返回空**：静默返回空会让用户以为账号全丢了，
/// 而实际文件还在、只是解析不了（还有救）。
pub fn load() -> Result<Vec<Account>> {
    let f = store_file();
    if !f.is_file() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&f)?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<Account> = serde_json::from_str(&raw).map_err(|e| {
        Error::Other(format!(
            "账号库解析失败（{}）：{}。文件仍完好，可手动修复或改名后重新导入",
            f.display(),
            e
        ))
    })?;
    Ok(list)
}

/// 原子写入全部账号。
pub fn save(list: &[Account]) -> Result<()> {
    let f = store_file();
    if let Some(parent) = f.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(list)?;

    // 原子替换：先写 .tmp，再 rename。
    // rename 在同一卷上是原子操作 —— 要么看到旧文件，要么看到新文件，
    // 不会看到"写了一半"的。
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, &f)?;
    Ok(())
}

/// 插入或更新一个账号（按 userid 去重）。
///
/// 返回 `true` 表示是**新增**（false = 更新已有）。
///
/// # 更新时的字段合并策略
///
/// 已有的**非空**字段优先保留 —— 用户自己填的 `note`/`name` 不该被
/// 一次重新导入冲掉。但 `session` 例外：它必须用新的（重新导入往往
/// 就是为了换一个新 session）。
pub fn upsert(list: &mut Vec<Account>, incoming: Account) -> bool {
    if let Some(existing) = list.iter_mut().find(|a| a.userid == incoming.userid) {
        existing.session = incoming.session;
        for (dst, src) in [
            (&mut existing.phone, &incoming.phone),
            (&mut existing.nickname, &incoming.nickname),
        ] {
            if !src.trim().is_empty() {
                *dst = src.clone();
            }
        }
        if existing.name.trim().is_empty() {
            existing.name = incoming.name;
        }
        if existing.imported_at == 0 {
            existing.imported_at = incoming.imported_at;
        }
        false
    } else {
        list.push(incoming);
        true
    }
}

/// 从任意形状的 JSON 里归一化出一个账号。
///
/// # 字段名兼容（这是**静默失败**的修补）
///
/// 不同来源用不同字段名：
///
/// ```text
/// 本工具导出      userid
/// 网关 auths      uid + userId
/// 别处            id
/// ```
///
/// 只认一个名字的后果是"导入 0 个且不报错"—— 用户只会怀疑文件给错了。
pub fn from_json(v: &serde_json::Value) -> Option<Account> {
    let obj = v.as_object()?;

    fn pick(o: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> String {
        for k in keys {
            match o.get(*k) {
                Some(serde_json::Value::String(s)) if !s.trim().is_empty() => {
                    return s.trim().to_string()
                }
                Some(serde_json::Value::Number(n)) => return n.to_string(),
                _ => {}
            }
        }
        String::new()
    }

    let userid = pick(obj, &["userid", "userId", "uid", "id"]);
    let session = pick(obj, &["session", "token", "accessToken", "access_token"]);
    if userid.is_empty() || session.is_empty() {
        return None;
    }
    Some(Account {
        userid,
        session,
        phone: pick(obj, &["phone", "mobile", "phoneNumber"]),
        nickname: pick(obj, &["nickname", "nickName", "displayName"]),
        name: pick(obj, &["name"]),
        note: String::new(),
        source: String::new(),
        imported_at: chrono::Utc::now().timestamp_millis(),
    })
}

/// 从一段文本解析账号（支持 JSON 对象 / JSON 字符串 / 裸 session）。
pub fn parse_text(text: &str) -> Result<Vec<Account>> {
    let t = text.trim();
    if t.is_empty() {
        return Err(Error::Invalid("内容为空".into()));
    }

    // ① JSON（对象、数组、或纯字符串）
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
        match &v {
            serde_json::Value::Array(items) => {
                let out: Vec<Account> = items.iter().filter_map(from_json).collect();
                if out.is_empty() {
                    return Err(Error::Invalid(
                        "JSON 数组里没有任何带 session 与 userid 的账号".into(),
                    ));
                }
                return Ok(out);
            }
            serde_json::Value::String(s) => {
                // {"..."}，裸 session 放在字符串里
                return parse_bare_session(s);
            }
            _ => {
                if let Some(a) = from_json(&v) {
                    return Ok(vec![a]);
                }
                let keys: Vec<&str> = v
                    .as_object()
                    .map(|o| o.keys().map(|s| s.as_str()).collect())
                    .unwrap_or_default();
                return Err(Error::Invalid(format!(
                    "缺少必需字段。找到的字段：{}。\n需要 session（或 token）与 userid（或 uid / userId）",
                    if keys.is_empty() {
                        "（无）".to_string()
                    } else {
                        keys.join(", ")
                    }
                )));
            }
        }
    }

    // ② 裸 session 串（32 位十六进制）
    parse_bare_session(t)
}

fn parse_bare_session(s: &str) -> Result<Vec<Account>> {
    let t = s.trim();
    let is_hex = t.len() >= 16 && t.len() <= 64 && t.chars().all(|c| c.is_ascii_hexdigit());
    if !is_hex {
        return Err(Error::Invalid(
            "无法解析：既不是 JSON，也不是纯 session 串（需要 16-64 位十六进制）".into(),
        ));
    }
    Err(Error::Invalid(format!(
        "只给了 session（{}…），缺少 userid。\n\n\
         userid 无法从 session 反推，请在 JSON 里一并提供，或从 Loomy 的\n\
         userData/auth-session.json 复制完整内容。",
        &t[..8.min(t.len())]
    )))
}

/// 备份目录。
pub fn backup_dir() -> PathBuf {
    paths::data_dir().join("backups")
}

/// 一次备份的记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMeta {
    pub name: String,
    pub created_at: i64,
    pub userid: String,
    pub session: String,
    /// 备份时 leveldb 里的登录态（可能没有）
    pub leveldb: Option<crate::session::leveldb::StoredSession>,
}

/// 列出备份（新的在前）。
pub fn list_backups() -> Vec<BackupMeta> {
    let dir = backup_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<BackupMeta> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.path().join("meta.json");
            let raw = std::fs::read_to_string(meta).ok()?;
            serde_json::from_str(&raw).ok()
        })
        .collect();
    // 新的在前（`Reverse` 而不是 `sort_by(|a,b| b.cmp(a))`，意图更直白）
    out.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    out
}

/// 写一份备份。
pub fn write_backup(m: &BackupMeta) -> Result<PathBuf> {
    let dir = backup_dir().join(&m.name);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(m)?)?;

    // 原始文件也存一份（出问题时能手工恢复）
    if let Some(root) = paths::find_loomy_roots().into_iter().next() {
        if let Ok(raw) = std::fs::read_to_string(&root.auth_file) {
            let _ = std::fs::write(dir.join("auth-session.json"), raw);
        }
    }
    Ok(dir)
}

/// 读备份的 meta。
pub fn read_backup(name: &str) -> Result<BackupMeta> {
    let f = backup_dir().join(name).join("meta.json");
    let raw =
        std::fs::read_to_string(&f).map_err(|_| Error::NotFound(format!("备份不存在: {name}")))?;
    Ok(serde_json::from_str(&raw)?)
}

/// 从目录批量导入（支持 `<dir>/auths/<上游>/*.json` 与 `<dir>/*.json`）。
///
/// 返回 (解析出的账号, 跳过原因) —— 跳过原因**必须**回给用户，
/// 静默跳过正是"我明明扫了却没进去"的成因。
pub fn import_from_dir(dir: &Path) -> (Vec<Account>, Vec<String>) {
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    scan_json_tree(dir, &mut found, &mut skipped, 0);
    // 同时试 <dir>/auths（网关的形态）
    let auths = dir.join("auths");
    if auths.is_dir() {
        scan_json_tree(&auths, &mut found, &mut skipped, 0);
    }
    (found, skipped)
}

fn scan_json_tree(dir: &Path, found: &mut Vec<Account>, skipped: &mut Vec<String>, depth: u32) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        let path = entry.path();
        if path.is_dir() {
            // 跳过隐藏目录（.rejected 之类的垃圾），最多下探 1 层
            if name.starts_with('.') || depth >= 1 {
                continue;
            }
            scan_json_tree(&path, found, skipped, depth + 1);
            continue;
        }
        if !name.ends_with(".json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(v) => {
                let items: Vec<serde_json::Value> = match v {
                    serde_json::Value::Array(a) => a,
                    other => vec![other],
                };
                for it in items {
                    match from_json(&it) {
                        Some(mut a) => {
                            a.source = path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            found.push(a);
                        }
                        None => {
                            skipped.push(format!("{}（缺 session 或 userid/uid）", path.display()))
                        }
                    }
                }
            }
            Err(e) => skipped.push(format!("{}（JSON 解析失败: {e}）", path.display())),
        }
    }
}
