//! Loomy 账号管理器 —— 启动壳。
//!
//! 业务代码全在 `lib.rs`（见那里的注释：为了可单元测试）。
//! 这里只负责调它。

// Windows 发布版不弹控制台窗口（debug 时保留，方便看日志）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    loomy_account_manager_lib::run();
}
