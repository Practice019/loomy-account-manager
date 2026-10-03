//! Loomy 积分网关客户端：余额、新手任务、邀请码。
//!
//! # 鉴权与模型代理**不是同一条轨**
//!
//! 本模块的接口只认请求头 **`token: <session>`** —— 实测它们不认
//! `Authorization: Bearer`（那是 `/chat/completions` 那条轨）。
//! 发错头会得到 401/404，而错误信息不会告诉你是头的问题。
//!
//! # 域名同源
//!
//! 积分网关与模型代理**同一个 host**（`loomyad.xunfei.cn`），
//! 只是路径前缀不同：
//!
//! ```text
//! 模型代理  https://loomyad.xunfei.cn/api/v1/chat/completions
//! 积分网关  https://loomyad.xunfei.cn/api/v1/points/records
//! ```
//!
//! # 端点清单（全部实测通过）
//!
//! ```text
//! GET  /points/records?pageNo=1&pageSize=1   余额
//! GET  /onboarding/tasks                     新手任务进度
//! POST /onboarding/tasks/complete           完成一个任务 {key}
//! GET  /points/activation                    激活状态（用了谁的邀请码）
//! POST /points/activation                    绑定邀请码 {invitationCode}
//! POST /points/first-login                   首登奖励
//! GET  /invitation-codes                     我生成的邀请码
//! ```

use serde::{Deserialize, Serialize};

use crate::core::error::{Error, Result};

/// 积分网关基址。
pub const BASE: &str = "https://loomyad.xunfei.cn/api/v1";

/// 上游回执信封：`{code, desc, data}`。
#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    code: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    data: serde_json::Value,
}

/// 发一次请求并解出 `data`。
///
/// # `code != 000000` 要原样带出来
///
/// `desc` 是上游给**人**看的话（"邀请码不存在"、"不能绑定自己账号生成的
/// 邀请码"…）。重写成我们自己的措辞会丢掉用户唯一能据此行动的线索。
async fn call(
    http: &reqwest::Client,
    session: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    if session.trim().is_empty() {
        return Err(Error::Invalid("缺少 session".into()));
    }
    let url = format!("{BASE}{path}");
    let mut req = http
        .request(method, &url)
        // ⚠ 只认 token 头（实测）
        .header("token", session)
        .header("Accept", "application/json");
    if let Some(b) = &body {
        req = req.header("Content-Type", "application/json").json(b);
    }

    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await?;

    let env: Envelope = serde_json::from_str(&text).map_err(|_| {
        Error::Other(format!(
            "积分接口回执不是合法 JSON（HTTP {status} {path}）：{}",
            crate::core::util::clip(&text, 300)
        ))
    })?;

    if !env.code.is_empty() && env.code != "000000" {
        return Err(Error::Upstream {
            code: env.code,
            desc: env.desc,
        });
    }
    Ok(env.data)
}

// ── 余额 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    /// 永久积分余额
    #[serde(default)]
    pub balance: i64,
    /// 当日积分余额（每天重置）
    #[serde(default)]
    pub daily_balance: i64,
    /// 可用总额（上面两者之和）—— 这才是"还能用多少"
    #[serde(default)]
    pub available_balance: i64,
    /// 流水总条数
    #[serde(default)]
    pub total: i64,
    /// 最近一条流水（便于显示"最后一次消耗了什么"）
    #[serde(default)]
    pub last_record: Option<LastRecord>,
}

/// 最近一条流水。
///
/// # 字段名与类型都是**实测**得来的（第一版是猜的，全错）
///
/// | 第一版（猜） | 上游实际 |
/// |---|---|
/// | `points: i64` | `pointsActual: i64` |
/// | `created_at: String` | `createdAt: i64`（Unix **秒**，是数字不是字符串） |
///
/// 类型错（数字给 `String`）会让整个 `from_value` 失败 → 调用处的
/// `.ok()` 吞掉错误 → `last_record` 恒为 `None` → 界面上"最近一笔"
/// 永远不显示，且不报任何错。`last_record_deserializes_from_real_sample`
/// 用一条真实流水把字段名和类型都钉住。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRecord {
    #[serde(default)]
    pub model_name: String,
    /// `"debit"`（扣）/ `"credit"`（加）
    #[serde(default)]
    pub direction: String,
    /// 上游叫 `pointsActual`
    #[serde(default, rename(deserialize = "pointsActual", serialize = "points"))]
    pub points: i64,
    /// Unix **秒**（不是毫秒，也不是字符串）
    #[serde(default)]
    pub created_at: i64,
    /// 上游给的说明文字（"模型调用扣分"）
    #[serde(default)]
    pub description: String,
}

/// 查余额。
pub async fn balance(http: &reqwest::Client, session: &str) -> Result<Balance> {
    let data = call(
        http,
        session,
        reqwest::Method::GET,
        "/points/records?pageNo=1&pageSize=1",
        None,
    )
    .await?;

    let mut b: Balance = serde_json::from_value(data.clone()).unwrap_or_default();
    // list[0] 摘成 last_record（上游把它放在 list 里）
    if let Some(first) = data
        .get("list")
        .and_then(|l| l.as_array())
        .and_then(|a| a.first())
    {
        b.last_record = serde_json::from_value(first.clone()).ok();
    }
    Ok(b)
}

// ── 新手任务 ──────────────────────────────────────────────────────────────

/// 任务奖励表（来自 Loomy 的 `onboarding-service.js` TASK_POINTS）。
///
/// 顺序**就是界面顺序** —— 用切片而不是 map，否则每次刷新顺序都变。
pub const TASK_DEFS: &[(&str, &str, i64)] = &[
    ("first_message", "首次对话", 500),
    ("pick_skill", "选择技能", 1000),
    ("generate_ppt", "生成 PPT", 1500),
    ("set_schedule", "设置日程", 1000),
    ("install_skill", "安装技能", 1500),
    ("configure_remote", "配置远程", 1000),
    ("create_soul", "创建人格", 1500),
    ("share_soul", "分享人格", 2000),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub key: String,
    pub label: String,
    pub points: i64,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub tasks: Vec<TaskItem>,
    pub earned: i64,
    pub total: i64,
}

/// 查新手任务进度。
pub async fn tasks(http: &reqwest::Client, session: &str) -> Result<TaskProgress> {
    let data = call(
        http,
        session,
        reqwest::Method::GET,
        "/onboarding/tasks",
        None,
    )
    .await?;

    let done = data.get("tasks").and_then(|t| t.as_object());
    let items = TASK_DEFS
        .iter()
        .map(|(key, label, points)| TaskItem {
            key: (*key).to_string(),
            label: (*label).to_string(),
            points: *points,
            done: done
                .and_then(|m| m.get(*key))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
        .collect();

    Ok(TaskProgress {
        tasks: items,
        earned: data.get("earned").and_then(|v| v.as_i64()).unwrap_or(0),
        total: data.get("total").and_then(|v| v.as_i64()).unwrap_or(0),
    })
}

/// 完成一个新手任务。
///
/// 返回 `(是否完成, 获得积分)`。
pub async fn complete_task(
    http: &reqwest::Client,
    session: &str,
    key: &str,
) -> Result<(bool, i64)> {
    let data = call(
        http,
        session,
        reqwest::Method::POST,
        "/onboarding/tasks/complete",
        Some(serde_json::json!({ "key": key })),
    )
    .await?;

    let ok = data
        .get("success")
        .and_then(|v| v.as_bool())
        .or_else(|| data.get("completed").and_then(|v| v.as_bool()))
        .unwrap_or(true); // 没有标志位但也没报错 → 视为成功
    let points = data
        .get("points")
        .and_then(|v| v.as_i64())
        .or_else(|| data.get("reward").and_then(|v| v.as_i64()))
        .unwrap_or_else(|| {
            TASK_DEFS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, _, p)| *p)
                .unwrap_or(0)
        });
    Ok((ok, points))
}

// ── 邀请码 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationState {
    /// 是否已激活（绑定过邀请码）
    #[serde(default)]
    pub activated: bool,
    /// 绑定的邀请码。上游叫 `appliedInvitationCode`，前端期望 `appliedCode`。
    /// 同 `InviteCode::code`：两个方向分开指定。
    #[serde(rename(deserialize = "appliedInvitationCode", serialize = "appliedCode"))]
    pub applied_code: String,
}

/// 查激活状态。
pub async fn activation(http: &reqwest::Client, session: &str) -> Result<ActivationState> {
    let data = call(
        http,
        session,
        reqwest::Method::GET,
        "/points/activation",
        None,
    )
    .await?;
    Ok(serde_json::from_value(data).unwrap_or(ActivationState {
        activated: false,
        applied_code: String::new(),
    }))
}

/// 绑定邀请码。
///
/// # 错误码（上游给的，原样透出给用户）
///
/// ```text
/// 100001  请求参数错误（码格式不对，或**绑自己的码**）
/// 200002  邀请码不存在
/// 200003  邀请码不可用（已被用掉/过期）
/// ```
pub async fn bind_invite(http: &reqwest::Client, session: &str, code: &str) -> Result<()> {
    let code = code.trim().to_uppercase();
    if code.is_empty() {
        return Err(Error::Invalid("邀请码为空".into()));
    }
    call(
        http,
        session,
        reqwest::Method::POST,
        "/points/activation",
        Some(serde_json::json!({ "invitationCode": code })),
    )
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteCode {
    /// 上游叫 `inviteCode`，但前端（`src/api/tauri.ts`）期望 `code`。
    ///
    /// # 为什么不能只写 `#[serde(rename = "inviteCode")]`
    ///
    /// 显式 `rename` 会**同时**作用于序列化与反序列化，覆盖 `rename_all`。
    /// 那样 Rust 会往前端发 `inviteCode`，前端读 `c.code` 得 `undefined`
    /// → 邀请码整列空白且不报错（截图里的现象）。
    ///
    /// 所以两个方向**分开指定**：收用上游名，发用前端名。
    /// `serialized_keys_match_frontend_expectations` 钉住这一点。
    #[serde(rename(deserialize = "inviteCode", serialize = "code"))]
    pub code: String,
    #[serde(default, rename = "usedCount")]
    pub used_count: i64,
    #[serde(default, rename = "maxUses")]
    pub max_uses: i64,
    /// 上游给的是 `"active"` / `"exhausted"`。
    ///
    /// ⚠ 第一版前端只判 `status === "available"`，而实际值是 `active`
    /// → **所有码都显示"已用完"**（截图里的现象）。前端改为按
    /// `active` 判断，并把非 active 显示为"不可用"。
    #[serde(default)]
    pub status: String,
}

/// 查我生成的邀请码（别人用我的码，我能拿奖励）。
pub async fn my_invite_codes(http: &reqwest::Client, session: &str) -> Result<Vec<InviteCode>> {
    let data = call(
        http,
        session,
        reqwest::Method::GET,
        "/invitation-codes",
        None,
    )
    .await?;
    Ok(data
        .get("list")
        .and_then(|l| l.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| serde_json::from_value::<InviteCode>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default())
}

/// 首登奖励（部分账号需要手动触发）。
pub async fn first_login(http: &reqwest::Client, session: &str) -> Result<()> {
    call(
        http,
        session,
        reqwest::Method::POST,
        "/points/first-login",
        Some(serde_json::json!({})),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_defs_total_is_10000() {
        let sum: i64 = TASK_DEFS.iter().map(|(_, _, p)| p).sum();
        assert_eq!(
            sum, 10000,
            "八个任务的奖励之和应当是 10000（与 Loomy 一致）"
        );
    }

    #[test]
    fn task_keys_are_unique() {
        let mut keys: Vec<&str> = TASK_DEFS.iter().map(|(k, _, _)| *k).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "任务 key 有重复");
    }

    #[test]
    fn balance_deserializes_from_real_shape() {
        // 实测回执形状
        let v = serde_json::json!({
            "balance": 2543,
            "dailyBalance": 973,
            "availableBalance": 3516,
            "pageNo": 1,
            "pageSize": 1,
            "total": 1992,
            "list": [{"modelName": "qwen3.8-flash", "direction": "debit"}]
        });
        let b: Balance = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(b.balance, 2543);
        assert_eq!(b.daily_balance, 973);
        assert_eq!(b.available_balance, 3516);
        assert_eq!(b.total, 1992);

        // `last_record` **不是** serde 填的 —— 它是 `balance()` 从 list[0]
        // 摘出来的。所以这里按函数的行为验，而不是指望反序列化顺带填上。
        // （我第一版就是错在这一点上：断言 deserialize 能填 last_record。）
        assert!(
            b.last_record.is_none(),
            "last_record 只由 balance() 填，不该被 serde 填"
        );
    }

    /// 用**一条真实流水**钉住 LastRecord 的字段名与类型。
    ///
    /// 这条抓的是：字段名猜错（`points` vs `pointsActual`）或类型给错
    ///（`String` vs `i64`）都会让 `from_value` 失败 → 调用处 `.ok()` 吞错
    /// → `last_record` 恒为 None → "最近一笔"永不显示，且不报错。
    #[test]
    fn last_record_deserializes_from_real_sample() {
        // 实测样本（原文照抄，含类型）
        let v = serde_json::json!({
            "ledgerId": "pl_fe442655a13d47c6b1e59da2c09746d5",
            "modelName": "GLM-5.3-Flash",
            "direction": "debit",
            "description": "模型调用扣分",
            "createdAt": 1791022091,
            "pointsActual": 34,
            "consumeSource": "daily",
            "balanceBefore": 13803,
            "balanceAfter": 13769,
            "dailyCycleDate": "2026-10-03"
        });
        let r: LastRecord = serde_json::from_value(v)
            .expect("真实样本必须能解析 —— 猜错字段名或类型就会在这里失败");

        assert_eq!(r.model_name, "GLM-5.3-Flash");
        assert_eq!(r.direction, "debit");
        assert_eq!(r.points, 34, "上游字段是 pointsActual");
        assert_eq!(r.created_at, 1791022091, "createdAt 是数字（Unix 秒）");
        assert_eq!(r.description, "模型调用扣分");

        // 发往前端的键名：前端读 lastRecord.modelName / .direction（有测试钉住）
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["modelName"], "GLM-5.3-Flash");
        assert_eq!(j["direction"], "debit");
        assert_eq!(j["points"], 34, "对前端暴露为 points（不是 pointsActual）");
    }

    /// 缺字段时不该整条失败（上游可能少给字段）。
    #[test]
    fn last_record_tolerates_missing_fields() {
        let v = serde_json::json!({"modelName": "x"});
        let r: LastRecord = serde_json::from_value(v).expect("缺字段应当走默认值");
        assert_eq!(r.model_name, "x");
        assert_eq!(r.points, 0);
        assert_eq!(r.created_at, 0);
    }

    /// list 为空时不该 panic（新账号没有流水）。
    #[test]
    fn balance_handles_empty_list() {
        let v = serde_json::json!({
            "balance": 0, "dailyBalance": 0, "availableBalance": 0, "total": 0, "list": []
        });
        let b: Balance = serde_json::from_value(v).unwrap();
        assert_eq!(b.available_balance, 0);
        assert!(b.last_record.is_none());
    }

    /// 缺字段时用默认值而不是报错（上游偶尔少给字段）。
    #[test]
    fn balance_tolerates_missing_fields() {
        let v = serde_json::json!({"balance": 100});
        let b: Balance = serde_json::from_value(v).unwrap();
        assert_eq!(b.balance, 100);
        assert_eq!(b.daily_balance, 0);
        assert_eq!(b.available_balance, 0);
    }

    #[test]
    fn activation_deserializes_applied_code() {
        let v = serde_json::json!({"activated": true, "appliedInvitationCode": "ZZ9999"});
        let a: ActivationState = serde_json::from_value(v).unwrap();
        assert!(a.activated);
        assert_eq!(a.applied_code, "ZZ9999");
    }

    #[test]
    fn invite_code_deserializes() {
        let v = serde_json::json!({"inviteCode": "E3HRN8", "usedCount": 1, "maxUses": 1, "status": "exhausted"});
        let c: InviteCode = serde_json::from_value(v).unwrap();
        assert_eq!(c.code, "E3HRN8");
        assert_eq!(c.status, "exhausted");
    }

    /// **序列化**后的字段名必须与前端 TypeScript 接口一致。
    ///
    /// # 为什么这条测试必须存在（它抓的是一个真实 bug）
    ///
    /// `#[serde(rename_all = "camelCase")]` 与 `#[serde(rename = "...")]`
    /// 组合时，显式 `rename` 会在**序列化和反序列化两个方向都生效**，
    /// 覆盖 `rename_all`。第一版就踩了：
    ///
    /// ```ignore
    /// #[serde(rename_all = "camelCase")]
    /// struct InviteCode {
    ///     #[serde(rename = "inviteCode")]   // ← 上游的字段名
    ///     code: String,                     // ← 前端期望 "code"
    /// }
    /// ```
    ///
    /// 后果：Rust 序列化出 `inviteCode`，前端读 `c.code` 得到 `undefined`
    /// → **邀请码那一列整列空白**，而且不报任何错（`undefined` 在 React 里
    /// 就是什么都不渲染）。截图里看到的正是这个。
    ///
    /// 所以这里把**发往前端的 JSON 键名**钉死。改字段名而忘了同步前端时，
    /// 这条会立刻红。
    #[test]
    fn serialized_keys_match_frontend_expectations() {
        // 前端 src/api/tauri.ts 的 InviteCode 接口
        let c = InviteCode {
            code: "AB12CD".into(),
            used_count: 0,
            max_uses: 1,
            status: "active".into(),
        };
        let j: serde_json::Value = serde_json::to_value(&c).unwrap();
        for key in ["code", "usedCount", "maxUses", "status"] {
            assert!(
                j.get(key).is_some(),
                "序列化结果缺少前端期望的字段 {key:?}（实际：{j}）"
            );
        }
        assert_eq!(j["code"], "AB12CD", "前端读 c.code，这里必须是 code");
        assert!(
            j.get("inviteCode").is_none(),
            "不该同时出现 inviteCode（前端不认识这个键）"
        );

        // 前端 ActivationState 接口
        let a = ActivationState {
            activated: true,
            applied_code: "ZZ9999".into(),
        };
        let j: serde_json::Value = serde_json::to_value(&a).unwrap();
        assert_eq!(j["activated"], true);
        assert_eq!(
            j["appliedCode"], "ZZ9999",
            "前端读 a.appliedCode（实际：{j}）"
        );
    }

    /// 反序列化仍要认上游的字段名（序列化名 ≠ 上游名，两个方向不能混）。
    #[test]
    fn deserialization_accepts_upstream_names() {
        let v = serde_json::json!({"inviteCode": "AB12CD", "usedCount": 0, "maxUses": 1, "status": "active"});
        let c: InviteCode = serde_json::from_value(v).unwrap();
        assert_eq!(c.code, "AB12CD");

        let v = serde_json::json!({"activated": true, "appliedInvitationCode": "ZZ9999"});
        let a: ActivationState = serde_json::from_value(v).unwrap();
        assert_eq!(a.applied_code, "ZZ9999");
    }

    #[test]
    fn tasks_deserialize_done_map() {
        let v = serde_json::json!({
            "tasks": {"configure_remote": true, "create_soul": false},
            "earned": 1000,
            "total": 10000
        });
        let done = v.get("tasks").and_then(|t| t.as_object()).unwrap();
        let items: Vec<TaskItem> = TASK_DEFS
            .iter()
            .map(|(key, label, points)| TaskItem {
                key: (*key).to_string(),
                label: (*label).to_string(),
                points: *points,
                done: done.get(*key).and_then(|x| x.as_bool()).unwrap_or(false),
            })
            .collect();
        assert_eq!(items.len(), 8);
        assert!(
            items
                .iter()
                .find(|t| t.key == "configure_remote")
                .unwrap()
                .done
        );
        assert!(!items.iter().find(|t| t.key == "create_soul").unwrap().done);
        // 没在 maps 里的任务应当算"未完成"而不是报错
        assert!(
            !items
                .iter()
                .find(|t| t.key == "first_message")
                .unwrap()
                .done
        );
    }
}
