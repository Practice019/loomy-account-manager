//! 统一错误类型。
//!
//! # 为什么不用 `Box<dyn Error>`
//!
//! 这些错误要**跨 Tauri IPC 边界**回给前端。`Box<dyn Error>` 没法
//! `serde::Serialize`，于是每处命令都得手写一遍转换（而漏一处就编译不过，
//! 逼着人去 `unwrap`）。用一个具体的枚举既好序列化，也能给前端**分类**判断
//! （例如"Loomy 正在运行"要提示用户关闭，而"文件不存在"不需要）。

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("网络请求失败: {0}")]
    Http(#[from] reqwest::Error),

    #[error("找不到: {0}")]
    NotFound(String),

    /// Loomy 没装，或找不到它的登录态位置。
    #[error("未找到本机 Loomy 安装：{0}")]
    LoomyNotFound(String),

    /// Loomy 正在运行 —— 切换账号必须先关掉它。
    ///
    /// 单列一类是因为**处理方式完全不同**：不是报错给用户看就算了，
    /// 而是要让前端弹"关闭并切换"的引导。
    #[error("Loomy 正在运行（PID {pids:?}）—— 切换账号前必须先完全退出它")]
    LoomyRunning { pids: Vec<u32> },

    /// 上游接口返回业务错误（code != 000000）。
    ///
    /// `desc` 是给**人**看的（"邀请码不存在"/"不能绑定自己账号生成的邀请码"），
    /// 原样带出来给用户，不要重写成我们自己的话。
    #[error("Loomy 接口错误 {code}: {desc}")]
    Upstream { code: String, desc: String },

    #[error("参数不合法: {0}")]
    Invalid(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// ── 与常见类型之间的便捷转换 ──────────────────────────────────────────────

impl From<PathBuf> for Error {
    fn from(p: PathBuf) -> Self {
        Error::NotFound(p.to_string_lossy().into_owned())
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

// ── 跨 Tauri IPC 边界 ─────────────────────────────────────────────────────
//
// # 为什么手写而不是 derive(Serialize)
//
// Tauri 要求命令的 `Err` 类型实现 `Serialize`。直接 derive 会把它序列化成
// 一个带 variant 名的复杂对象（`{"Io":{"msg":...}}`），前端得写 switch
// 才能取到文本。
//
// **为什么必须手写**：JSON 序列化过程中 `serde_json::Error` 与
// `reqwest::Error` 都不是 `Serialize`，derive 会让那些变体编译不过；
// 而更重要的是 —— 前端只需要**一段能显示的中文**。所以这里统一转成
// 一个带 `kind` + `message` 的扁平对象：
//
// ```json
// {"kind":"loomyRunning","message":"Loomy 正在运行（PID [123]）…"}
// ```
//
// `kind` 让前端能对**特定类别**做不同处理（例如 `loomyRunning` 弹
// "关闭并切换"的引导，而 `upstream` 直接显示上游原话）。
impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Error", 3)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        // 结构化附加信息（前端按需取用）
        let extra = match self {
            Error::LoomyRunning { pids } => Some(serde_json::json!({ "pids": pids })),
            Error::Upstream { code, desc } => {
                Some(serde_json::json!({ "code": code, "desc": desc }))
            }
            _ => None,
        };
        st.serialize_field("detail", &extra)?;
        st.end()
    }
}

impl Error {
    /// 机器可读的类别（前端据此决定怎么展示）。
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Json(_) => "json",
            Error::Http(_) => "http",
            Error::NotFound(_) => "notFound",
            Error::LoomyNotFound(_) => "loomyNotFound",
            Error::LoomyRunning { .. } => "loomyRunning",
            Error::Upstream { .. } => "upstream",
            Error::Invalid(_) => "invalid",
            Error::Other(_) => "other",
        }
    }
}
