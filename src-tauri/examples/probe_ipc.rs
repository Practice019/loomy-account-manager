//! 验证 IPC 载荷 —— 把真实接口数据按 Tauri 的方式序列化后打印。
//!
//! # 为什么需要这个（它抓的是一个真实 bug）
//!
//! 单元测试用的是**手写**的 struct；这条用的是**线上真实数据**经完整
//! `fetch → serialize` 链路。两者不同：真实数据可能带来单元测试没覆盖的
//! 字段值（例如 `status: "active"` 而不是我以为的 `"available"`）。
//!
//! 打印的 JSON 就是前端 `invoke()` 实际收到的对象。字段名或取值不对，
//! 一眼能看出来。
//!
//! 用法：cargo run --example probe_ipc

use loomy_account_manager_lib::clients::{http, points};
use loomy_account_manager_lib::core::account;

#[tokio::main]
async fn main() {
    let list = account::load().expect("读账号库失败");
    let Some(acct) = list.first() else {
        eprintln!("库是空的，先跑：cargo run --example seed");
        std::process::exit(1);
    };
    println!("账号: {} ({})\n", acct.userid, acct.phone);
    let c = http::shared();

    // ── 邀请码：这是出过 bug 的地方 ────────────────────────────────────
    println!("═══ fetch_my_invites 的 IPC 载荷 ═══");
    match points::my_invite_codes(&c, &acct.session).await {
        Ok(codes) => {
            // Tauri 就是把返回值这样序列化后交给前端的
            let payload = serde_json::to_string_pretty(&codes).expect("序列化失败");
            println!("{payload}");

            // 逐项断言前端会用到的键与值
            let first = &codes[0];
            println!("\n前端读 c.code        → {:?}", first.code);
            println!("前端读 c.usedCount   → {}", first.used_count);
            println!("前端读 c.maxUses     → {}", first.max_uses);
            println!("前端读 c.status      → {:?}", first.status);

            assert!(!first.code.is_empty(), "code 为空 → 前端那一列会空白");

            // 状态取值必须与前端判断一致
            let known = ["active", "exhausted"];
            for x in &codes {
                assert!(
                    known.contains(&x.status.as_str()),
                    "出现未知 status {:?}；前端的判断只认 {known:?}",
                    x.status
                );
            }
            let active = codes.iter().filter(|x| x.status == "active").count();
            println!(
                "\n✓ {} 个码，其中 {} 个 active（前端显示为可用）",
                codes.len(),
                active
            );
        }
        Err(e) => println!("✗ 失败: {e}"),
    }

    // ── 激活状态 ───────────────────────────────────────────────────────
    println!("\n═══ fetch_activation 的 IPC 载荷 ═══");
    match points::activation(&c, &acct.session).await {
        Ok(a) => {
            println!("{}", serde_json::to_string_pretty(&a).unwrap());
            println!("\n前端读 a.activated   → {}", a.activated);
            println!("前端读 a.appliedCode → {:?}", a.applied_code);
        }
        Err(e) => println!("✗ 失败: {e}"),
    }

    // ── 余额 ───────────────────────────────────────────────────────────
    println!("\n═══ fetch_balance 的 IPC 载荷 ═══");
    match points::balance(&c, &acct.session).await {
        Ok(b) => {
            println!("{}", serde_json::to_string_pretty(&b).unwrap());
            println!("\n前端读 availableBalance → {}", b.available_balance);
        }
        Err(e) => println!("✗ 失败: {e}"),
    }

    // ── 任务 ───────────────────────────────────────────────────────────
    println!("\n═══ fetch_tasks 的 IPC 载荷（前 1 项）═══");
    match points::tasks(&c, &acct.session).await {
        Ok(p) => {
            println!("tasks[0] = {}", serde_json::to_string(&p.tasks[0]).unwrap());
            println!("earned={} total={}", p.earned, p.total);
            for t in &p.tasks {
                assert!(!t.key.is_empty() && !t.label.is_empty());
            }
            println!("\n前端读 t.done / t.label / t.points → 都非空 ✓");
        }
        Err(e) => println!("✗ 失败: {e}"),
    }
}
