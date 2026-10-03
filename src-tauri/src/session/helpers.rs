//! 会话相关的便捷入口（给命令层用）。
//!
//! 这些函数只是把"拿第一个 Loomy 安装 / 第一个 leveldb 目录"这种
//! 常见组合包一层 —— 命令层不该自己拼这些路径。

use crate::core::error::Result;
use crate::core::paths;

/// 读本机当前激活账号的 session（找不到返回 None）。
pub fn paths_active_session() -> Option<String> {
    let root = paths::find_loomy_roots().into_iter().next()?;
    super::read_active(&root).map(|a| a.session)
}

/// 读第一个 leveldb 目录里的当前登录态。
///
/// 没有 leveldb 目录、或目录里没有登录态 → `Ok(None)`
/// （这是**合法状态**，例如从没登录过，不是错误）。
pub fn read_current_session_from_default() -> Result<Option<super::leveldb::StoredSession>> {
    let Some(dir) = paths::find_leveldb_dirs().into_iter().next() else {
        return Ok(None);
    };
    let Some(log) = paths::current_leveldb_log(&dir) else {
        return Ok(None);
    };
    super::leveldb::read_current_session(&log)
}
