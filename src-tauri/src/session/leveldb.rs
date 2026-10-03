//! 写 Chromium 的 localStorage（LevelDB），使 Loomy 认新账号。
//!
//! # 为什么必须写这里（本工具踩过的坑）
//!
//! 切换账号时只写 `auth-session.json` + `opencode.json` **不生效**。
//! 追踪 Loomy 渲染进程（`app.asar` → `dist/assets/sonner-*.js`，函数 `mM`）：
//!
//! ```js
//! function mM(){
//!   const e = F1()                                 // 读 localStorage["loomy-auth-session"]
//!   return window.electronAPI.auth.setSession({    // 推给主进程
//!     session: e.session, userid: e.userid, phone: e.phone
//!   })
//! }
//! ```
//!
//! **方向是反的**：渲染进程把自己 localStorage 里的值推给主进程，
//! 主进程据此**覆盖** `auth-session.json`。所以真实链路是：
//!
//! ```text
//! 1. 我们写 auth-session.json = 新账号
//! 2. Loomy 启动 → 渲染进程读它**自己 localStorage 里的旧值**
//! 3. 调 auth.setSession() → 主进程 update() → 把 auth-session.json 改回旧账号
//! ```
//!
//! 实测时间线证实（16:11:56 写入 → 16:13:23 启动 → 16:13:28 被改回）。
//!
//! # LevelDB 日志格式（本模块自己实现，不引 LevelDB 依赖）
//!
//! 文件由 **32768 字节的块**组成，块内是记录：
//!
//! ```text
//! ┌─────────┬──────────┬───────┬──────────────────┐
//! │ crc32c  │ length   │ type  │ data             │
//! │ 4 LE    │ 2 LE     │ 1     │ length bytes     │
//! └─────────┴──────────┴───────┴──────────────────┘
//!
//! crc = mask(crc32c(type_byte || data))
//! type: 1 全量 / 2 首块 / 3 中块 / 4 尾块
//! ```
//!
//! 每块数据是一个 **WriteBatch**：
//!
//! ```text
//! sequence (8 LE) | count (4 LE) | records...
//! record: type(1: 1=PUT 0=DEL) | varint(klen) | key | varint(vlen) | val
//! ```
//!
//! # 两个容易错的细节（都实测确认过）
//!
//! 1. **key 里没有 origin 前缀**。网上很多 Chromium 教程写
//!    `_<origin>\x00\x01<key>`，但 Loomy 的 localStorage 只有一个来源，
//!    Chromium 于是省略 origin：实际是 `_file://\x00\x01loomy-auth-session`。
//!    照抄带 origin 的格式会写出一条**永远读不到**的记录。
//!
//! 2. **CRC 必须带掩码**（`MaskCrc`）。漏了会让 Chromium 判记录损坏，
//!    最坏情况整个 localStorage 被丢弃。

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::core::error::{Error, Result};

const BLOCK_SIZE: usize = 32_768;

/// LevelDB 的 CRC 掩码（见 `db/format.h` 的 `MaskCrc`）。
///
/// `((crc >> 15) | (crc << 17)) + 0xa282ead8`
fn mask_crc(crc: u32) -> u32 {
    crc.rotate_right(15).wrapping_add(0xa282_ead8)
}

/// localStorage 条目的 key：`_file://` + `\x00\x01` + 名字（**无 origin**）。
fn ls_key(name: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(11 + name.len());
    k.extend_from_slice(b"_file://");
    k.extend_from_slice(&[0x00, 0x01]);
    k.extend_from_slice(name.as_bytes());
    k
}

/// 要写进 localStorage 的登录态（与 Loomy 自己的 JSON 形状一致）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSession {
    pub phone: String,
    pub masked_phone: String,
    pub session: String,
    pub userid: String,
    pub logged_in_at: String,
}

/// 从 leveldb 里读出的当前登录态（原文 JSON）。
pub fn read_current_session(log: &Path) -> Result<Option<StoredSession>> {
    let mut f = File::open(log)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let parsed = parse_log(&buf);
    // 取**最后一条** PUT（后来的覆盖先前的）
    let key = ls_key("loomy-auth-session");
    let mut found: Option<StoredSession> = None;
    for batch in &parsed.batches {
        for (t, k, v) in &batch.entries {
            if k == &key {
                if *t == EntryType::Del {
                    found = None;
                } else if let Some(val) = v {
                    // value 前缀 \x01 是 Chromium 的"非空"标记
                    let json = if val.first() == Some(&0x01) {
                        &val[1..]
                    } else {
                        &val[..]
                    };
                    if let Ok(s) = serde_json::from_slice::<StoredSession>(json) {
                        found = Some(s);
                    }
                }
            }
        }
    }
    Ok(found)
}

/// 把登录态写进 leveldb。
///
/// 返回 `true` 表示确实写了（`false` = 已经是目标值，跳过）。
///
/// # 调用前必须关闭 Loomy
///
/// 客户端运行中时它内存里有 localStorage 副本，退出时会把旧值刷回磁盘 ——
/// 我们的写入会被覆盖。调用方（命令层）必须先检测进程。
pub fn write_session(log: &Path, s: &StoredSession) -> Result<bool> {
    // 已是目标值 → 不写（幂等，避免日志无意义增长）
    if let Some(cur) = read_current_session(log)? {
        if cur.session == s.session && cur.userid == s.userid {
            return Ok(false);
        }
    }

    let mut buf = Vec::new();
    File::open(log)?.read_to_end(&mut buf)?;

    let parsed = parse_log(&buf);
    let end = end_of_records(&buf);
    let seq = parsed.max_seq + 1;

    let value = {
        let json = serde_json::to_vec(s)?;
        let mut v = Vec::with_capacity(json.len() + 1);
        v.push(0x01); // Chromium 的"value 非空"标记
        v.extend_from_slice(&json);
        v
    };
    // 与 Loomy 自己的写入一致：同时更新"最近登录手机号"
    let last_phone = {
        let mut v = vec![0x01];
        v.extend_from_slice(s.phone.as_bytes());
        v
    };

    let entries = vec![
        WriteEntry::put(ls_key("loomy-auth-session"), value),
        WriteEntry::put(ls_key("loomy-last-login-phone"), last_phone),
    ];

    let chunks = encode_batch(&entries, seq, end);

    let mut f = OpenOptions::new().read(true).write(true).open(log)?;
    let mut pos = end as u64;
    for c in &chunks {
        match c {
            Chunk::Pad(n) => {
                f.seek(SeekFrom::Start(pos))?;
                f.write_all(&vec![0u8; *n])?;
                pos += *n as u64;
            }
            Chunk::Data(bytes) => {
                f.seek(SeekFrom::Start(pos))?;
                f.write_all(bytes)?;
                pos += bytes.len() as u64;
            }
        }
    }
    f.flush()?;
    f.sync_all()?;
    Ok(true)
}

// ── 解析 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryType {
    Put,
    Del,
}

#[derive(Debug, Default)]
struct ParsedBatch {
    entries: Vec<(EntryType, Vec<u8>, Option<Vec<u8>>)>,
}

#[derive(Debug, Default)]
struct ParsedLog {
    batches: Vec<ParsedBatch>,
    max_seq: u64,
    /// CRC 校验统计（测试用）
    crc_ok: usize,
    crc_bad: usize,
}

fn read_varint(buf: &[u8], mut p: usize) -> (u64, usize) {
    let mut result: u64 = 0;
    let mut shift = 0;
    while p < buf.len() {
        let b = buf[p];
        p += 1;
        result |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 63 {
            break;
        }
    }
    (result, p)
}

fn write_varint(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while n >= 0x80 {
        out.push(((n & 0x7f) as u8) | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
    out
}

/// 解析整个日志（含 CRC 校验）。
fn parse_log(buf: &[u8]) -> ParsedLog {
    let mut out = ParsedLog::default();
    let mut pending: Vec<u8> = Vec::new();
    let mut in_frag = false;
    let mut off = 0usize;

    while off + 7 <= buf.len() {
        let block_off = off % BLOCK_SIZE;
        // 块尾不足 7 字节放不下 header → 跳到下一块
        if block_off > BLOCK_SIZE - 7 {
            off += BLOCK_SIZE - block_off;
            continue;
        }
        let crc = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
        let len = u16::from_le_bytes([buf[off + 4], buf[off + 5]]) as usize;
        let ty = buf[off + 6];
        if len == 0 && ty == 0 {
            off += 7;
            continue;
        }
        if off + 7 + len > buf.len() {
            break;
        }
        let data = &buf[off + 7..off + 7 + len];

        // CRC 校验：mask(crc32c(type || data))
        let mut for_crc = Vec::with_capacity(len + 1);
        for_crc.push(ty);
        for_crc.extend_from_slice(data);
        if mask_crc(crc32c::crc32c(&for_crc)) == crc {
            out.crc_ok += 1;
        } else {
            out.crc_bad += 1;
        }

        match ty {
            1 => {
                if let Some(b) = parse_batch(data, &mut out.max_seq) {
                    out.batches.push(b);
                }
            }
            2 => {
                pending = data.to_vec();
                in_frag = true;
            }
            3 => {
                if in_frag {
                    pending.extend_from_slice(data);
                }
            }
            4 => {
                if in_frag {
                    pending.extend_from_slice(data);
                    if let Some(b) = parse_batch(&pending, &mut out.max_seq) {
                        out.batches.push(b);
                    }
                }
                pending.clear();
                in_frag = false;
            }
            _ => {}
        }
        off += 7 + len;
    }
    out
}

fn parse_batch(data: &[u8], max_seq: &mut u64) -> Option<ParsedBatch> {
    if data.len() < 12 {
        return None;
    }
    let seq = u64::from_le_bytes(data[0..8].try_into().ok()?);
    if seq > *max_seq {
        *max_seq = seq;
    }
    let count = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
    let mut p = 12usize;
    let mut batch = ParsedBatch::default();
    for _ in 0..count {
        if p >= data.len() {
            break;
        }
        let t = data[p];
        p += 1;
        let (klen, np) = read_varint(data, p);
        p = np;
        let klen = klen as usize;
        if p + klen > data.len() {
            break;
        }
        let key = data[p..p + klen].to_vec();
        p += klen;
        let mut val = None;
        if t == 1 {
            let (vlen, np2) = read_varint(data, p);
            p = np2;
            let vlen = vlen as usize;
            if p + vlen > data.len() {
                break;
            }
            val = Some(data[p..p + vlen].to_vec());
            p += vlen;
        } else {
            // DEL 记录：LevelDB 的 batch 里 DEL 也带 varint(0) 的长度占位
            let (_zero, np2) = read_varint(data, p);
            p = np2;
        }
        batch.entries.push((
            if t == 1 {
                EntryType::Put
            } else {
                EntryType::Del
            },
            key,
            val,
        ));
    }
    Some(batch)
}

/// 找出记录区的真实末尾（不能直接用文件长度：可能有块尾填充）。
fn end_of_records(buf: &[u8]) -> usize {
    let mut off = 0usize;
    while off + 7 <= buf.len() {
        let block_off = off % BLOCK_SIZE;
        if block_off > BLOCK_SIZE - 7 {
            off += BLOCK_SIZE - block_off;
            continue;
        }
        let len = u16::from_le_bytes([buf[off + 4], buf[off + 5]]) as usize;
        let ty = buf[off + 6];
        if len == 0 && ty == 0 {
            break; // 零填充区，写在这里会破坏格式
        }
        if off + 7 + len > buf.len() {
            break;
        }
        off += 7 + len;
    }
    off
}

// ── 编码 ──────────────────────────────────────────────────────────────────

struct WriteEntry {
    key: Vec<u8>,
    value: Vec<u8>,
}

impl WriteEntry {
    fn put(key: Vec<u8>, value: Vec<u8>) -> Self {
        Self { key, value }
    }
}

enum Chunk {
    Data(Vec<u8>),
    Pad(usize),
}

/// 把一个 WriteBatch 编成若干日志记录（按 32 KiB 块分片）。
fn encode_batch(entries: &[WriteEntry], seq: u64, start_offset: usize) -> Vec<Chunk> {
    let mut body = Vec::new();
    for e in entries {
        body.push(1u8); // PUT
        body.extend_from_slice(&write_varint(e.key.len() as u64));
        body.extend_from_slice(&e.key);
        body.extend_from_slice(&write_varint(e.value.len() as u64));
        body.extend_from_slice(&e.value);
    }
    let mut batch = Vec::with_capacity(12 + body.len());
    batch.extend_from_slice(&seq.to_le_bytes());
    batch.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    batch.extend_from_slice(&body);

    let mut chunks = Vec::new();
    let mut remaining: &[u8] = &batch;
    let mut pos = start_offset;
    let mut is_first = true;

    while !remaining.is_empty() {
        let block_off = pos % BLOCK_SIZE;
        let avail = BLOCK_SIZE - block_off;
        if avail < 7 {
            chunks.push(Chunk::Pad(avail));
            pos += avail;
            continue;
        }
        let take = std::cmp::min(avail - 7, remaining.len());
        let piece = &remaining[..take];
        let is_last = take == remaining.len();
        let ty: u8 = if is_first {
            if is_last {
                1
            } else {
                2
            }
        } else if is_last {
            4
        } else {
            3
        };
        let mut for_crc = Vec::with_capacity(piece.len() + 1);
        for_crc.push(ty);
        for_crc.extend_from_slice(piece);
        let crc = mask_crc(crc32c::crc32c(&for_crc));

        let mut rec = Vec::with_capacity(7 + piece.len());
        rec.extend_from_slice(&crc.to_le_bytes());
        rec.extend_from_slice(&(piece.len() as u16).to_le_bytes());
        rec.push(ty);
        rec.extend_from_slice(piece);
        chunks.push(Chunk::Data(rec));

        pos += 7 + piece.len();
        remaining = &remaining[take..];
        is_first = false;
    }
    chunks
}

/// 把手机号转成脱敏形式（`150****3411`），与 Loomy 的 `Zb()` 一致。
pub fn mask_phone(phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 11 {
        format!("{}****{}", &digits[..3], &digits[7..])
    } else {
        phone.to_string()
    }
}

/// 写登录态到**所有**找到的 leveldb 目录。
///
/// 返回实际写入的日志路径列表。
pub fn write_session_all(s: &StoredSession) -> Result<Vec<PathBuf>> {
    let dirs = crate::core::paths::find_leveldb_dirs();
    if dirs.is_empty() {
        return Err(Error::NotFound(
            "找不到 Loomy 的 localStorage 目录（%APPDATA%\\Loomy\\Local Storage\\leveldb）".into(),
        ));
    }
    let mut written = Vec::new();
    for dir in dirs {
        let Some(log) = crate::core::paths::current_leveldb_log(&dir) else {
            continue;
        };
        if write_session(&log, s)? {
            written.push(log);
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 用一个真实文件校验解析器：CRC 必须**全部**通过。
    ///
    /// 这条测试的意义：CRC 算法（含掩码）如果写错，解析器会静默把每条
    /// 记录判成损坏 —— 而"读得出数据"却"CRC 全错"是最难发现的状态。
    /// 拿真机文件跑一次就能确定算法对不对。
    #[test]
    fn parses_real_file_with_valid_crc() {
        let dirs = crate::core::paths::find_leveldb_dirs();
        let Some(dir) = dirs.first() else {
            eprintln!("跳过：本机没有 Loomy localStorage");
            return;
        };
        let Some(log) = crate::core::paths::current_leveldb_log(dir) else {
            eprintln!("跳过：leveldb 目录里没有 .log");
            return;
        };
        let mut buf = Vec::new();
        File::open(&log).unwrap().read_to_end(&mut buf).unwrap();
        let parsed = parse_log(&buf);
        assert!(parsed.crc_ok > 0, "一条记录都没解析出来");
        assert_eq!(
            parsed.crc_bad, 0,
            "CRC 校验失败 {} 条（通过 {}）—— 说明 CRC 算法或掩码写错了",
            parsed.crc_bad, parsed.crc_ok
        );
    }

    /// 往返：造一个临时日志 → 写 → 读回，值和 CRC 都要对。
    #[test]
    fn write_then_read_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("ldb-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let log = tmp.join("000003.log");
        // 造一个**空**日志文件（没有历史记录 → maxSeq 从 0 开始）
        fs::write(&log, b"").unwrap();

        let s = StoredSession {
            phone: "13800000001".into(),
            masked_phone: mask_phone("13800000001"),
            session: "0123456789abcdef0123456789abcdef".into(),
            userid: "260101000000000001".into(),
            logged_in_at: "2026-10-03T08:17:04.091Z".into(),
        };

        let wrote = write_session(&log, &s).unwrap();
        assert!(wrote, "首次写入应当返回 true");

        let back = read_current_session(&log).unwrap().expect("应当读回来");
        assert_eq!(back.session, s.session);
        assert_eq!(back.userid, s.userid);
        assert_eq!(back.phone, s.phone);
        assert_eq!(back.masked_phone, "138****0001");

        // CRC 必须全通过
        let mut buf = Vec::new();
        File::open(&log).unwrap().read_to_end(&mut buf).unwrap();
        let parsed = parse_log(&buf);
        assert_eq!(parsed.crc_bad, 0, "写入的记录 CRC 不合法");

        // 幂等：同一个值再写一次应当跳过
        let again = write_session(&log, &s).unwrap();
        assert!(!again, "已经是目标值时不应当重复写");

        let _ = fs::remove_dir_all(&tmp);
    }

    /// 切换账号：两次写入不同 session，读回来应是**后**一个。
    #[test]
    fn later_write_wins() {
        let tmp = std::env::temp_dir().join(format!("ldb-test2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let log = tmp.join("000003.log");
        fs::write(&log, b"").unwrap();

        let mk = |uid: &str, sess: &str| StoredSession {
            phone: "13800000000".into(),
            masked_phone: "138****0000".into(),
            session: sess.into(),
            userid: uid.into(),
            logged_in_at: "2026-10-03T00:00:00.000Z".into(),
        };

        write_session(&log, &mk("u1", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")).unwrap();
        write_session(&log, &mk("u2", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")).unwrap();

        let back = read_current_session(&log).unwrap().unwrap();
        assert_eq!(back.userid, "u2", "后写的应当获胜");
        assert_eq!(back.session, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        let mut buf = Vec::new();
        File::open(&log).unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(parse_log(&buf).crc_bad, 0);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn masks_phone_like_loomy() {
        assert_eq!(mask_phone("13812345678"), "138****5678");
        assert_eq!(mask_phone("13800000000"), "138****0000");
        // 非 11 位不脱敏（原样返回）
        assert_eq!(mask_phone("150****3411"), "150****3411");
    }

    /// 已脱敏的手机号再次脱敏不应变成乱码。
    #[test]
    fn mask_is_stable_on_masked_input() {
        let once = mask_phone("13812345678");
        assert_eq!(mask_phone(&once), once);
    }

    /// 跨块写入：值足够大时会分片（type 2/3/4），CRC 仍须正确。
    #[test]
    fn handles_block_boundary() {
        let tmp = std::env::temp_dir().join(format!("ldb-test3-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let log = tmp.join("000001.log");
        // 先塞一个接近块尾的大小，逼出分片
        let mut filler = Vec::new();
        for i in 0..80 {
            let key = ls_key(&format!("filler-key-{i}"));
            let val = vec![0x01u8; 400];
            let e = WriteEntry::put(key, val);
            filler.extend_from_slice(&write_varint(e.key.len() as u64));
            filler.extend_from_slice(&e.key);
            filler.extend_from_slice(&write_varint(e.value.len() as u64));
            filler.extend_from_slice(&e.value);
        }
        let mut batch = Vec::new();
        batch.extend_from_slice(&1u64.to_le_bytes());
        batch.extend_from_slice(&80u32.to_le_bytes());
        batch.extend_from_slice(&filler);
        // 手工封成一个 type=1 记录
        let mut for_crc = vec![1u8];
        for_crc.extend_from_slice(&batch);
        let crc = mask_crc(crc32c::crc32c(&for_crc));
        let mut rec = Vec::new();
        rec.extend_from_slice(&crc.to_le_bytes());
        rec.extend_from_slice(&(batch.len() as u16).to_le_bytes());
        rec.push(1);
        rec.extend_from_slice(&batch);
        fs::write(&log, &rec).unwrap();

        let s = StoredSession {
            phone: "13800000000".into(),
            masked_phone: "138****0000".into(),
            session: "cccccccccccccccccccccccccccccccc".into(),
            userid: "u9".into(),
            logged_in_at: "2026-10-03T00:00:00.000Z".into(),
        };
        write_session(&log, &s).unwrap();

        let mut buf = Vec::new();
        File::open(&log).unwrap().read_to_end(&mut buf).unwrap();
        let parsed = parse_log(&buf);
        assert_eq!(parsed.crc_bad, 0, "跨块写入后 CRC 必须仍全部合法");
        let back = read_current_session(&log).unwrap().unwrap();
        assert_eq!(back.userid, "u9");

        let _ = fs::remove_dir_all(&tmp);
    }
}
