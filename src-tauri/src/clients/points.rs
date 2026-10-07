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
//! POST /points/activation                    绑定邀请码 {inviteCode, deviceId}
//! POST /points/first-login                   首登奖励 {deviceId}
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationState {
    /// 是否已激活（绑定过邀请码）
    #[serde(default)]
    pub activated: bool,
    /// 绑定的邀请码。上游叫 `appliedInvitationCode`，前端期望 `appliedCode`。
    /// 同 `InviteCode::code`：两个方向分开指定。
    #[serde(rename(deserialize = "appliedInvitationCode", serialize = "appliedCode"))]
    #[serde(default)]
    pub applied_code: String,
}

/// 查激活状态。
///
/// # 为什么要在两层里找 `activated`
///
/// 回执可能是 `{"activated":…}`，也可能多包一层 `{"data":{"activated":…}}`
/// （上游有 v2 风格的外层包装先例，见 `my_invite_codes`）。
/// 只在第一层找的话，遇到包装过的那种会**静默**退化成"未激活" —— 于是
/// 界面显示"未激活"，而账号其实已经激活了，用户会以为绑定失败。
pub async fn activation(http: &reqwest::Client, session: &str) -> Result<ActivationState> {
    let data = call(
        http,
        session,
        reqwest::Method::GET,
        "/points/activation",
        None,
    )
    .await?;

    for layer in [Some(data.clone()), nested_data(&data)] {
        let Some(layer) = layer else { continue };
        // 用 Option 承接 `activated`，才能区分"字段存在且为 false"
        // 与"字段不存在"（前者是答案，后者说明要找下一层）。
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Probe {
            #[serde(default)]
            activated: Option<bool>,
            #[serde(default, rename = "appliedInvitationCode")]
            applied_code: String,
        }
        let Ok(p) = serde_json::from_value::<Probe>(layer) else {
            continue;
        };
        if let Some(activated) = p.activated {
            return Ok(ActivationState {
                activated,
                applied_code: p.applied_code,
            });
        }
    }
    // 回执里没有 activated 字段（老账号/异常回执）：当作未激活，不报错。
    Ok(ActivationState::default())
}

/// 取 `{"data":{…}}` 里那一层；没有就返回 `None`。
///
/// 上游部分端点在 v2 风格下会多包一层，两种形状都要认。
fn nested_data(v: &serde_json::Value) -> Option<serde_json::Value> {
    let inner = v.get("data")?;
    if inner.is_null() {
        return None;
    }
    Some(inner.clone())
}

/// 绑邀请码时上游要求的设备标识（协议要求，见 `device_id`）。
///
/// 客户端（Loomy 自己）用 `loomy-campus-<uuid>` 这个形状。它**只用于风控归因**，
/// 不参与鉴权；所以不复用同一个值 —— 复用会让多个账号被上游识别成同一台设备。
const DEVICE_PREFIX: &str = "loomy-campus-";

/// 生成一个新的 deviceId。
fn new_device_id() -> String {
    format!("{DEVICE_PREFIX}{}", uuid::Uuid::new_v4())
}

/// 绑定邀请码的请求体。
///
/// 抽成函数是为了让 `bind_payload_uses_upstream_field_names` 能钉住字段名 ——
/// 这个字段名写错时上游只回一句笼统的 `100001 请求参数错误`，
/// 不会说"你字段名错了"，现象是"绑定不了"（真踩过）。
fn bind_payload(code: &str) -> serde_json::Value {
    serde_json::json!({
        "inviteCode": code,
        "deviceId": new_device_id(),
    })
}

/// 把绑定邀请码的上游错误翻译成人话。
///
/// # 为什么需要翻译
///
/// 上游错误码对用户毫无意义 —— 三种失败原因（自绑 / 抄错 / 已用过）
/// 分别回 `100001`/`200002`/`200003`，光看数字和那句笼统的 desc
/// 完全看不出"码为什么不行"，用户会以为是自己操作错了或者程序有 bug。
///
/// 实测错误码：
///
/// ```text
/// 100001  请求参数错误  —— 自绑（拿自己生成的码绑自己）
/// 200002  邀请码不存在  —— 抄错 / 多复制了字符
/// 200003  邀请码不可用  —— 已用过（maxUses=1）/ 已失效
/// ```
///
/// 未命中时保留原文，并补一句通用提示（界面不该只有一行晦涩报错）。
fn translate_bind_error(e: Error) -> Error {
    let raw = e.to_string();
    const HINT: &str = "请确认用的是其它账号「我生成的邀请码」里可用的码，且该码还没被用过。";
    let friendly = if raw.contains("100001") {
        Some(format!("不能绑定自己账号生成的邀请码。{HINT}"))
    } else if raw.contains("200002") {
        Some(format!(
            "邀请码不存在：可能抄错了字符或多复制了内容。{HINT}"
        ))
    } else if raw.contains("200003") {
        Some(format!("邀请码不可用：已被使用或已失效。{HINT}"))
    } else {
        None
    };
    match friendly {
        Some(msg) => Error::Other(msg),
        None => Error::Other(format!("{raw} —— {HINT}")),
    }
}

/// 绑定邀请码。
///
/// # 请求体字段名是 `inviteCode`，不是 `invitationCode`
///
/// 这里踩过一个坑：第一版发的是 `{"invitationCode": code}` —— 读起来更"对"
/// （上游**读**激活状态时返回的字段确实叫 `appliedInvitationCode`），
/// 但**写**接口不认这个名字。实测：
///
/// ```text
/// {"invitationCode":"ZZZZZZ"}                  -> 100001 请求参数错误
/// {"inviteCode":"ZZZZZZ","deviceId":"..."}    -> 000000 成功
/// ```
///
/// 读接口与写接口的字段名**不一致**，而错的那一侧只会得到一个笼统的
/// `100001 请求参数错误`，不会说"字段名错了" —— 于是现象是"绑定不了"。
///
/// `deviceId` 是协议要求的（客户端会带）。实测省略它服务端**也**接受，
/// 但为与官方客户端一致、并让风控归因正确，这里照带上。
pub async fn bind_invite(http: &reqwest::Client, session: &str, code: &str) -> Result<()> {
    let code = code.trim().to_uppercase();
    if code.is_empty() {
        return Err(Error::Invalid("邀请码为空".into()));
    }
    // 长度只做**粗**校验：实测是 6 位，但不按"必须 6 位"硬拒 ——
    // 上游才是权威，将来码变长时这里会误杀。明显不对的长度（1 位 / 50 位）
    // 交给上游回 200002。
    if code.len() < 4 || code.len() > 32 {
        return Err(Error::Invalid(
            "邀请码长度不对（实测 6 位，形如 E3HRN8）".into(),
        ));
    }
    call(
        http,
        session,
        reqwest::Method::POST,
        "/points/activation",
        Some(bind_payload(&code)),
    )
    .await
    .map_err(translate_bind_error)?;
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
    ///
    /// 反序列化同时接受 `code`（上游换过名，两种都见过）。
    #[serde(rename(deserialize = "inviteCode", serialize = "code"))]
    #[serde(alias = "code")]
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
///
/// # 形状兼容三种（实测是第一种，其余为防御）
///
/// ```text
/// {"list":[{inviteCode,…}]}       ← 当前实测
/// {"data":{"list":[…]}}           ← v2 风格的外层包装
/// [{…}] 或 ["ABC123"]             ← 裸数组 / 纯字符串元素
/// ```
///
/// 只认第一种的话，上游换包装时会**静默**返回空列表 —— 界面显示"没有邀请码"，
/// 而接口其实明明返回了。这与 `activation` 的两层查找是同一类防御。
pub async fn my_invite_codes(http: &reqwest::Client, session: &str) -> Result<Vec<InviteCode>> {
    let data = call(
        http,
        session,
        reqwest::Method::GET,
        "/invitation-codes",
        None,
    )
    .await?;

    let list = extract_invite_list(&data);

    Ok(list
        .iter()
        .filter_map(|v| {
            // 元素可能是纯字符串（"ABC123"），也可能是个对象。
            if let Some(s) = v.as_str() {
                let s = s.trim();
                if s.is_empty() {
                    return None;
                }
                return Some(InviteCode {
                    code: s.to_string(),
                    used_count: 0,
                    max_uses: 1,
                    status: "active".into(),
                });
            }
            serde_json::from_value::<InviteCode>(v.clone()).ok()
        })
        .collect())
}

/// 从三种可能的形状里取出「邀请码」那一列元素。
///
/// ```text
/// {"list":[…]}          ← 当前实测
/// {"data":{"list":[…]}} ← v2 风格外层包装
/// […]                   ← 裸数组
/// ```
fn extract_invite_list(data: &serde_json::Value) -> Vec<serde_json::Value> {
    let inner = nested_data(data);
    data.get("list")
        .and_then(|l| l.as_array())
        .or_else(|| {
            inner
                .as_ref()
                .and_then(|d| d.get("list"))
                .and_then(|l| l.as_array())
        })
        .or_else(|| data.as_array())
        .cloned()
        .unwrap_or_default()
}

/// 首登奖励（部分账号需要手动触发）。
///
/// # 为什么需要它
///
/// 客户端**每次登录后**会自动调本端点完成积分账号初始化：服务端下发注册奖励
/// 并**生成 5 个邀请码**。通过本工具导入的账号跳过了这一步，表现就是
/// `invitation-codes` 返回空列表（"我生成的码"空）且注册奖励没到账。
/// 实测对空账号调用后立刻拿到 5 个码。
///
/// 服务端有 `alreadyProcessed` 幂等保护，重复调用不会重复发奖。
pub async fn first_login(http: &reqwest::Client, session: &str) -> Result<()> {
    call(
        http,
        session,
        reqwest::Method::POST,
        "/points/first-login",
        Some(serde_json::json!({ "deviceId": new_device_id() })),
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

    /// **绑定请求体的字段名必须是 `inviteCode`**（不是 `invitationCode`）。
    ///
    /// # 这条测试抓的是一个真实 bug（"绑定不了邀请码"）
    ///
    /// 第一版发的是 `{"invitationCode": code}` —— 名字读起来更"对"，
    /// 因为**读**激活状态的回执里字段确实叫 `appliedInvitationCode`。
    /// 但**写**接口不认这个名字。对线上实测的对照：
    ///
    /// ```text
    /// {"invitationCode":"ZZZZZZ"}                 -> 100001 请求参数错误
    /// {"inviteCode":"ZZZZZZ","deviceId":"..."}   -> 000000 成功
    /// ```
    ///
    /// 症状是"绑定不了"，而返回的错误只说"请求参数错误"，根本不提字段名 ——
    /// 从现象无法反推原因。所以这里把字段名钉死。
    #[test]
    fn bind_payload_uses_upstream_field_names() {
        let p = bind_payload("AB12CD");

        assert_eq!(
            p.get("inviteCode").and_then(|v| v.as_str()),
            Some("AB12CD"),
            "上游写接口认 inviteCode（实际：{p}）"
        );
        assert!(
            p.get("invitationCode").is_none(),
            "必须是 inviteCode —— invitationCode 会让上游回 100001（实际：{p}）"
        );

        // deviceId 是协议要求的，形状为 loomy-campus-<uuid>
        let d = p
            .get("deviceId")
            .and_then(|v| v.as_str())
            .expect("绑定请求必须带 deviceId");
        assert!(
            d.starts_with("loomy-campus-"),
            "deviceId 形状应为 loomy-campus-<uuid>（实际：{d}）"
        );
        assert!(
            d.len() > "loomy-campus-".len() + 30,
            "deviceId 后面应当是一个 UUID（实际：{d}）"
        );
    }

    /// 每次生成的 deviceId 必须不同。
    ///
    /// 复用同一个会让上游把多个账号识别成同一台设备（风控归因错误）。
    #[test]
    fn device_id_is_fresh_each_time() {
        let a = new_device_id();
        let b = new_device_id();
        assert_ne!(a, b, "deviceId 不该复用");
        assert!(a.starts_with("loomy-campus-"));
    }

    /// 上游的晦涩错误码要翻译成人话。
    ///
    /// 不翻译的话，用户看到的就是"绑定失败：积分接口 /points/activation
    /// 返回 100001: 请求参数错误" —— 完全看不出该怎么办。
    #[test]
    fn bind_errors_are_translated_to_human_text() {
        let cases = [
            ("100001", "不能绑定自己账号生成的邀请码"),
            ("200002", "邀请码不存在"),
            ("200003", "邀请码不可用"),
        ];
        for (code, expect) in cases {
            let e = Error::Upstream {
                code: code.into(),
                desc: "请求参数错误".into(),
            };
            let msg = translate_bind_error(e).to_string();
            assert!(
                msg.contains(expect),
                "错误码 {code} 应翻译出 {expect:?}（实际：{msg}）"
            );
            // 每种都要带可行动的提示，不能只有一句话
            assert!(
                msg.contains("我生成的邀请码"),
                "错误码 {code} 的提示应当告诉用户去哪找码（实际：{msg}）"
            );
        }
    }

    /// 没命中已知错误码时，要保留上游原文（别把线索丢了）。
    #[test]
    fn unknown_bind_error_keeps_original_text() {
        let e = Error::Other("网络超时".into());
        let msg = translate_bind_error(e).to_string();
        assert!(msg.contains("网络超时"), "未命中的错误必须保留原文：{msg}");
    }

    /// 长度粗校验：明显不对的直接拒（不发请求），可疑长度放给上游判。
    #[test]
    fn bind_length_guard_is_loose_not_strict() {
        // 这些明显不对，应当本地就拒
        for bad in ["A", "AB", "ABC"] {
            assert!(
                bad.len() < 4,
                "测试数据本身要短于 4（{bad}）—— 这是本地拒绝的下界"
            );
        }
        // 6 位是实测长度，必须放行给上游
        let ok = "E3HRN8";
        assert!(
            ok.len() >= 4 && ok.len() <= 32,
            "{ok} 应当放行给上游（不因长度被本地拒）"
        );
        // 不能写死"必须 6 位"—— 那会在上游改长度时误杀
        let longer = "ABCDEFGH";
        assert!(
            longer.len() >= 4 && longer.len() <= 32,
            "8 位码不该被本地拒（上游才是权威）"
        );
    }

    /// 回执多包一层 `{"data":{…}}` 时也要认（否则静默显示"未激活"）。
    #[test]
    fn activation_state_found_in_nested_data() {
        let flat = serde_json::json!({"activated": true, "appliedInvitationCode": "V5GZ3M"});
        let nested =
            serde_json::json!({"data": {"activated": true, "appliedInvitationCode": "V5GZ3M"}});

        assert_eq!(nested_data(&flat), None, "没有 data 层时应为 None");
        let inner = nested_data(&nested).expect("应当取到 data 那一层");
        assert_eq!(inner["activated"], true);
    }

    /// `activated` 存在且为 false 是**有效答案**，不能继续往下一层找。
    #[test]
    fn activation_false_is_a_real_answer() {
        let v = serde_json::json!({"activated": false, "appliedInvitationCode": ""});
        let probe: Option<bool> = v.get("activated").and_then(|x| x.as_bool());
        assert_eq!(
            probe,
            Some(false),
            "false 与「字段不存在」必须能区分 —— 否则未激活会被判成要找下一层"
        );
    }

    /// `invitation-codes` 的形状容错：list / data.list / 裸数组 / 字符串元素。
    #[test]
    fn invite_codes_tolerate_all_shapes() {
        // 当前实测形状
        let a = serde_json::json!({"list": [{"inviteCode": "E3HRN8", "status": "exhausted"}]});
        assert_eq!(extract_invite_list(&a).len(), 1);
        // v2 风格外层包装
        let b = serde_json::json!({"data": {"list": [{"inviteCode": "SSK5MC"}]}});
        assert_eq!(extract_invite_list(&b).len(), 1);
        // 裸数组
        let c = serde_json::json!([{"inviteCode": "3PAXHB"}]);
        assert_eq!(extract_invite_list(&c).len(), 1);
        // 空回执不能 panic
        assert!(extract_invite_list(&serde_json::json!({})).is_empty());
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
