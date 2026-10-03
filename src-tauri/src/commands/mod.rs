//! Tauri 命令层 —— 前端通过 `invoke()` 调的就是这些。
//!
//! # 这一层只做三件事
//!
//! 1. **参数校验**（把用户的输入变成明确的错误，而不是让下游 panic）
//! 2. **调用下层**（`core` / `session` / `clients`）
//! 3. **把结果整理成前端好用的形状**
//!
//! 业务逻辑不写在这里 —— 那样它就没法在单元测试里跑了
//! （命令函数要 Tauri 运行时）。

pub mod account_cmd;
pub mod meta_cmd;
pub mod switch_cmd;
pub mod system_cmd;
