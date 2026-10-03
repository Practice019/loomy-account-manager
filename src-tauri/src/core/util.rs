//! 小工具。

/// 把文本截短（日志/错误信息用）。
///
/// 按 **char** 边界截，不是字节 —— 按字节切 UTF-8 会 panic，
/// 而错误信息里带中文是常态。
pub fn clip(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// 时间戳（毫秒）→ 本地时间字符串。
pub fn fmt_time(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "-".into())
}

/// 生成备份目录名（`2026-10-03T08-19-30`，文件系统安全）。
pub fn backup_name() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_respects_char_boundary() {
        // 中文按字节切会 panic；这里必须安全
        let s = "电商产品主图宣传海报，正方形1:1构图";
        let c = clip(s, 5);
        assert_eq!(c, "电商产品主…");
        // 不超长时不该加省略号
        assert_eq!(clip("abc", 10), "abc");
        assert_eq!(clip("abc", 3), "abc");
    }

    #[test]
    fn clip_handles_empty() {
        assert_eq!(clip("", 5), "");
    }

    #[test]
    fn backup_name_is_filesystem_safe() {
        let n = backup_name();
        assert!(!n.contains(':'), "冒号在 Windows 文件名里非法: {n}");
        assert!(n.len() >= 19, "格式应当像 2026-10-03T08-19-30: {n}");
    }
}
