//! 核心：路径、账号模型、错误、工具。
//!
//! 这一层**不认识 Tauri**（不 import `tauri::*`）也不认识 UI ——
//! 它只处理"文件在哪、账号是什么形状"。这样它能在单元测试里直接跑，
//! 不必启动整个应用。

pub mod account;
pub mod config;
pub mod error;
pub mod paths;
pub mod util;
