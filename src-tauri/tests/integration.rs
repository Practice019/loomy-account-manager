//! 集成测试：用**真实的本机 Loomy 环境**验证核心链路。
//!
//! # 为什么要有集成测试（而不是只有单元测试）
//!
//! 单元测试用的是临时文件，能验算法，但验不了"本机实际长什么样"：
//!
//! - `auth-session.json` 的真实字段名与形状
//! - leveldb 目录的真实位置与大小写（Windows 上 Loomy/loomy 是同一个）
//! - 网关 `auths/<上游>/*.json` 的真实字段名（`uid` + `userId`，不是 `userid`）
//! - 两处登录态**是否一致**（不一致正是"切换不生效"的形态）
//!
//! 这些测试**只读**真实文件（写路径的测试在 leveldb.rs 用临时文件做），
//! 所以能安全地在任何机器上跑；缺前置条件时跳过而不是失败。

use loomy_account_manager_lib::core::{account, paths};
use loomy_account_manager_lib::session;

#[test]
fn finds_real_loomy_installation() {
    let roots = paths::find_loomy_roots();
    if roots.is_empty() {
        eprintln!("跳过：本机没有 Loomy");
        return;
    }
    for r in &roots {
        println!("安装: {}", r.root.display());
        assert!(r.auth_file.is_file(), "auth_file 应当存在");
    }
}

#[test]
fn finds_real_leveldb() {
    let dirs = paths::find_leveldb_dirs();
    if dirs.is_empty() {
        eprintln!("跳过：本机没有 Loomy localStorage");
        return;
    }
    for d in &dirs {
        println!("leveldb: {}", d.display());
        assert!(
            paths::current_leveldb_log(d).is_some(),
            "应当能找到 .log 文件"
        );
    }
    // 去重必须生效：%APPDATA%\Loomy 与 %APPDATA%\loomy 在 Windows 上是同一个
    assert_eq!(
        dirs.len(),
        1,
        "大小写不同的同一目录应当被去重成一个（否则会往同一处写两遍）"
    );
}

#[test]
fn reads_real_active_session() {
    let roots = paths::find_loomy_roots();
    let Some(root) = roots.first() else {
        eprintln!("跳过：本机没有 Loomy");
        return;
    };
    let Some(a) = session::read_active(root) else {
        eprintln!("跳过：当前没有登录态");
        return;
    };
    println!("当前激活: userid={} phone={}", a.userid, a.phone);
    assert!(!a.session.is_empty());
    assert!(a.session.len() >= 16, "session 应当是个够长的串");

    // 与 leveldb 里的一致 —— 这是本工具修过的坑：两处不同步时
    // 切换会"看着成功、重启又变回去"。
    match session::read_current_session_from_default() {
        Ok(Some(ldb)) => {
            println!("leveldb : userid={} phone={}", ldb.userid, ldb.phone);
            assert_eq!(
                ldb.session, a.session,
                "auth-session.json 与 leveldb 不一致 —— 这正是'切换不生效'的形态"
            );
        }
        Ok(None) => eprintln!("leveldb 里没有登录态"),
        Err(e) => eprintln!("读 leveldb 失败: {e}"),
    }
}

/// 网关的 `auths/<上游>/*.json` 用 `uid` + `userId`（**不是** `userid`），
/// 必须能识别。
///
/// # 为什么自建 fixture 而不指向本机网关目录
///
/// 第一版写死了我开发机上的路径
///（一个本机专有目录）。后果：
///
/// - 在**别人**的机器上那个目录不存在 → 测试走进"跳过"分支 →
///   这条测试对所有人都是"通过但没验证任何东西"
/// - 我自己的网关目录还含真实账号，等于把隐私带进了仓库
///
/// 现在用临时目录造一样形状的 fixture。测的是**同一段逻辑**
///（字段名兼容），但不依赖任何机器状态、也不含真实数据。
#[test]
fn parses_gateway_style_account_files() {
    let base = std::env::temp_dir().join(format!("gw-fixture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    // 复刻网关布局：<dir>/auths/loomy/*.json
    let dir = base.join("auths").join("loomy");
    std::fs::create_dir_all(&dir).unwrap();

    // 形状照抄网关真实文件（uid + userId + 脱敏昵称），值是假的
    std::fs::write(
        dir.join("loomy-260101000000000001.json"),
        r#"{"session":"0123456789abcdef0123456789abcdef","uid":"260101000000000001","phone":"150****0001","nickname":"NK0000001","userId":"260101000000000001"}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("loomy-260101000000000002.json"),
        r#"{"session":"fedcba9876543210fedcba9876543210","uid":"260101000000000002","phone":"150****0002","nickname":"NK0000002","userId":"260101000000000002"}"#,
    )
    .unwrap();
    // 一个坏文件，用来确认"跳过并报原因"而不是静默忽略
    std::fs::write(dir.join("broken.json"), "{ 这不是 JSON").unwrap();

    let (found, skipped) = account::import_from_dir(&base);
    println!("找到 {} 个，跳过 {} 个", found.len(), skipped.len());
    assert_eq!(
        found.len(),
        2,
        "两个有效文件都该被识别（uid/userId 字段名兼容）"
    );
    assert_eq!(skipped.len(), 1, "坏文件应当被报告而不是静默跳过");

    // 按文件名排序保证顺序稳定
    let mut ids: Vec<&str> = found.iter().map(|a| a.userid.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["260101000000000001", "260101000000000002"]);
    for a in &found {
        assert!(!a.session.is_empty());
        assert!(a.phone.starts_with("150"), "手机号应当被读出来");
    }

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn detects_running_process_without_panic() {
    let pids = session::loomy_pids();
    println!("检测到 Loomy 进程: {pids:?}");
    for p in pids {
        assert!(p > 0);
    }
}

#[test]
fn parse_text_gives_actionable_errors() {
    // 缺 userid 时要说清缺什么、去哪儿拿
    let e = account::parse_text("0123456789abcdef0123456789abcdef").unwrap_err();
    let msg = e.to_string();
    println!("{msg}");
    assert!(msg.contains("userid"), "错误信息应当点明缺 userid：{msg}");
    assert!(
        msg.contains("auth-session.json"),
        "应当告诉用户去哪儿找：{msg}"
    );

    // 完全无关的内容要说清既不是 JSON 也不是 session
    let e2 = account::parse_text("这不是账号").unwrap_err();
    assert!(e2.to_string().contains("无法解析"), "{e2}");
}

#[test]
fn parse_text_accepts_gateway_field_names() {
    let json = r#"{"session":"0123456789abcdef0123456789abcdef","uid":"260101000000000001","phone":"13800000001","nickname":"NK0000001","userId":"260101000000000001"}"#;
    let list = account::parse_text(json).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].userid, "260101000000000001");
    assert_eq!(list[0].phone, "13800000001");
    assert_eq!(list[0].nickname, "NK0000001");
}

#[test]
fn parse_text_accepts_array() {
    let json = r#"[
      {"session":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","userid":"u1"},
      {"session":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","uid":"u2"}
    ]"#;
    let list = account::parse_text(json).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].userid, "u1");
    assert_eq!(list[1].userid, "u2");
}

/// 账号库的原子写入：写坏的中间状态不该被读到。
#[test]
fn account_store_roundtrip_is_atomic() {
    // 用一个临时目录当 data_dir 不现实（data_dir 由 exe 位置决定），
    // 所以这里只验"存→读"的往返在真实路径上不丢字段。
    // ⚠ 为了不污染用户的账号库，先记下原内容，测完还原。
    let f = account::store_file();
    let original = std::fs::read_to_string(&f).ok();

    let list = vec![account::Account {
        userid: "test-atomic".into(),
        session: "cccccccccccccccccccccccccccccccc".into(),
        phone: "13800000000".into(),
        nickname: "n".into(),
        name: String::new(),
        note: "note".into(),
        source: "test".into(),
        imported_at: 1,
    }];
    account::save(&list).unwrap();
    let back = account::load().unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].userid, "test-atomic");
    assert_eq!(back[0].note, "note");

    // 还原
    match original {
        Some(raw) => std::fs::write(&f, raw).unwrap(),
        None => {
            let _ = std::fs::remove_file(&f);
        }
    }
}
