//! 共享 HTTP 客户端。
//!
//! # 为什么要一个共享的
//!
//! `reqwest::Client` 内部维护连接池。每次调用都 `Client::new()` 会**每次
//! 重新握手 TLS** —— 查 13 个账号的余额就是 13 次完整握手，慢且没必要。
//! 一个 `OnceLock` 里的实例就够了。

use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// 取共享客户端。
pub fn shared() -> reqwest::Client {
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                // 超时交给单次请求控制，这里给一个宽松上限兜底。
                // 上游慢的时候（查余额偶尔要 2-3 秒）不该被 10 秒掐掉，
                // 但也不能无限等 —— 30 秒足够，且 UI 侧还有自己的 loading 态。
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .user_agent(concat!("loomy-account-manager/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("构造 HTTP 客户端失败")
        })
        .clone()
}
