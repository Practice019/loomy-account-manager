//! 端到端：导入 → 切换 → 验证三处一致 → 还原。
//!
//! # 为什么用 `#[ignore]` 而不是删掉
//!
//! 这条会**真的改本机 Loomy 的登录态**（虽然最后还原）。放在默认测试集里
//! 会让任何一次 `cargo test` 都去动用户的登录状态 —— 那是不可接受的副作用。
//!
//! 所以标 `#[ignore]`，需要时显式跑：
//!
//! ```bash
//! # 需要先关闭 Loomy（切换会拒绝在它运行时执行）
//! cargo test --test e2e_switch -- --ignored --nocapture
//! ```
//!
//! 它会：
//! 1. 记下当前账号
//! 2. 切到另一个（库里第二个）
//! 3. 断言 auth-session.json 与 leveldb 都变成新的
//! 4. 还原回原账号
//! 5. 再断言三处一致
//!
//! 任何一步失败都会**尽力还原**（用 `catch_unwind` 包住）。

use loomy_account_manager_lib::core::{account, paths};
use loomy_account_manager_lib::session;

/// 跳过条件：没有 Loomy、没有 leveldb、Loomy 正在运行。
fn ready() -> Option<(paths::LoomyRoot, Vec<account::Account>)> {
    let roots = paths::find_loomy_roots();
    let Some(root) = roots.into_iter().next() else {
        eprintln!("跳过：没找到 Loomy 安装");
        return None;
    };
    if paths::find_leveldb_dirs().is_empty() {
        eprintln!("跳过：没找到 leveldb");
        return None;
    }
    let pids = session::loomy_pids();
    if !pids.is_empty() {
        eprintln!("跳过：Loomy 正在运行（{pids:?}）—— 先关闭它再跑这条测试");
        return None;
    }
    let list = account::load().unwrap_or_default();
    if list.len() < 2 {
        eprintln!("跳过：库里少于 2 个账号（需要两个才能来回切）");
        return None;
    }
    Some((root, list))
}

#[test]
#[ignore = "会真的改本机 Loomy 登录态（测完还原），需显式 --ignored 运行"]
fn switch_changes_all_three_locations_and_restores() {
    let Some((root, list)) = ready() else { return };

    // ① 记下当前状态
    let before = session::read_active(&root).expect("当前应当有登录态");
    let before_ldb = session::read_current_session_from_default()
        .ok()
        .flatten()
        .expect("leveldb 里应当有登录态");
    println!(
        "切换前：auth-session={} leveldb={}",
        before.userid, before_ldb.userid
    );
    assert_eq!(
        before.session, before_ldb.session,
        "起点就不同步，先修这个再测"
    );

    // 挑一个**不同**的账号
    let target = list
        .iter()
        .find(|a| a.session != before.session)
        .expect("应当有一个不同的账号");

    // ② 切换（把还原动作放进一个兜底 closure）
    let restore = |reason: &str| {
        eprintln!("还原中（{reason}）…");
        let ok = account::Account {
            userid: before.userid.clone(),
            session: before.session.clone(),
            phone: before.phone.clone(),
            nickname: String::new(),
            name: String::new(),
            note: String::new(),
            source: "restore".into(),
            imported_at: 0,
        };
        match session::apply(&root, &ok) {
            Ok(_) => eprintln!("还原完成"),
            Err(e) => eprintln!("⚠ 还原失败，请手动切回：{e}"),
        }
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let rep = session::apply(&root, target).expect("切换应当成功");
        println!(
            "切换报告：json={} opencode={:?} leveldb={:?}",
            rep.json_written, rep.opencode_files, rep.leveldb_logs
        );

        // ③ 断言三处都变了
        assert!(rep.json_written, "auth-session.json 应当已写");
        assert!(
            !rep.leveldb_logs.is_empty(),
            "leveldb 应当已写 —— 没写的话切换不会生效"
        );

        let after = session::read_active(&root).expect("应当能读到新登录态");
        assert_eq!(
            after.userid, target.userid,
            "auth-session.json 没变成目标账号"
        );

        let after_ldb = session::read_current_session_from_default()
            .ok()
            .flatten()
            .expect("leveldb 应当能读到");
        assert_eq!(
            after_ldb.userid, target.userid,
            "leveldb 没变成目标账号 —— 这正是'切换不生效'的根因"
        );
        assert_eq!(
            after_ldb.session, target.session,
            "leveldb 的 session 与目标不符"
        );

        println!("✓ 三处都已切换：{} → {}", before.userid, target.userid);
        println!("  auth-session.json = {}", after.session);
        println!("  leveldb           = {}", after_ldb.session);
    }));

    // ④ 无论成败都还原
    match result {
        Ok(()) => {
            restore("测试通过");
            // ⑤ 还原后必须三处一致
            let back = session::read_active(&root).unwrap();
            let back_ldb = session::read_current_session_from_default()
                .unwrap()
                .unwrap();
            assert_eq!(
                back.session, before.session,
                "还原后 auth-session.json 不对"
            );
            assert_eq!(back_ldb.session, before.session, "还原后 leveldb 不对");
            println!("✓ 已还原到 {}", before.userid);
        }
        Err(e) => {
            restore("测试失败");
            std::panic::resume_unwind(e);
        }
    }
}
