//! 开发工具：实测积分接口（余额 / 任务 / 邀请码）。
//!
//! 用法：cargo run --example probe_points [userid]

use loomy_account_manager_lib::clients::{http, points};
use loomy_account_manager_lib::core::account;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list = account::load().expect("读账号库失败");
    if list.is_empty() {
        eprintln!("库是空的，先跑：cargo run --example seed");
        std::process::exit(1);
    }
    let acct = match args.first() {
        Some(uid) => list.iter().find(|a| &a.userid == uid),
        None => list.first(),
    }
    .expect("找不到该账号");

    println!("账号: {} ({})", acct.userid, acct.phone);
    let c = http::shared();

    match points::balance(&c, &acct.session).await {
        Ok(b) => println!(
            "✓ 余额: 可用 {} = 永久 {} + 当日 {}{}",
            b.available_balance,
            b.balance,
            b.daily_balance,
            b.last_record
                .as_ref()
                .map(|r| format!("  最近: {} {}", r.model_name, r.direction))
                .unwrap_or_default()
        ),
        Err(e) => println!("✗ 余额失败: {e}"),
    }

    match points::tasks(&c, &acct.session).await {
        Ok(p) => {
            let done = p.tasks.iter().filter(|t| t.done).count();
            println!(
                "✓ 任务: {}/{} 完成，已得 {}/{}",
                done,
                p.tasks.len(),
                p.earned,
                p.total
            );
            for t in &p.tasks {
                println!(
                    "    {} {} (+{})",
                    if t.done { "✓" } else { "○" },
                    t.label,
                    t.points
                );
            }
        }
        Err(e) => println!("✗ 任务失败: {e}"),
    }

    match points::activation(&c, &acct.session).await {
        Ok(a) => println!(
            "✓ 激活: {} {}",
            a.activated,
            if a.applied_code.is_empty() {
                String::new()
            } else {
                format!("（用了 {}）", a.applied_code)
            }
        ),
        Err(e) => println!("✗ 激活状态失败: {e}"),
    }

    match points::my_invite_codes(&c, &acct.session).await {
        Ok(codes) => {
            println!("✓ 我的邀请码: {} 个", codes.len());
            for x in codes.iter().take(5) {
                println!(
                    "    {} {}/{} {}",
                    x.code, x.used_count, x.max_uses, x.status
                );
            }
        }
        Err(e) => println!("✗ 邀请码失败: {e}"),
    }

    // ── 绑定请求体形状校验（非破坏性）─────────────────────────────────────
    //
    // 发一个**故意不存在**的码。它不可能绑成功，所以不改动任何状态；
    // 但上游的反应能证明请求体是否被正确解析：
    //
    //   200002 邀请码不存在   → 字段名对了（上游读到了 code，只是查不到）
    //   100001 请求参数错误   → 字段名错了（上游根本没读到 code）
    //
    // 这正是"邀请码绑定不了"那个 bug 的判据 —— 用 `probe_points bind-check`
    // 单独跑，因为要发一次写请求。
    if args.iter().any(|a| a == "bind-check") {
        println!("\n--- 绑定请求体形状校验 ---");
        println!("发一个**故意不存在**的码（ZZZZZZ）。它不可能绑成功，所以不改动任何状态，");
        println!("但上游的反应能证明请求体是否被正确解析。\n");

        match points::bind_invite(&c, &acct.session, "ZZZZZZ").await {
            Ok(()) => {
                // 走到这里说明上游回了 000000。对这个账号而言，这**就是**有效判据：
                // 参数校验发生在"已激活短路"之前 —— 同一个账号发
                // `invitationCode` 会得到 100001（参数校验失败），发 `inviteCode`
                // 才得到 000000。所以 000000 证明字段名被接受了。
                match points::activation(&c, &acct.session).await {
                    Ok(a) if a.activated => println!(
                        "✓ 上游回 000000 —— 请求体参数校验通过（字段名正确）。\n  \
                         该账号已激活（用了 {}），上游随后短路返回、不再校验码本身。\n  \
                         判据仍然有效：参数校验在短路**之前** —— 发 invitationCode 时\n  \
                         同一账号会得到 100001（参数校验失败），只有 inviteCode 能过。",
                        a.applied_code
                    ),
                    _ => println!(
                        "✓ 上游回 000000，且账号未激活 —— 请求体被正确解析（字段名正确）。"
                    ),
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("200002") {
                    println!("✓ 上游回 200002（邀请码不存在）");
                    println!("  → 字段名正确：上游读到了 inviteCode，只是查不到这个码。");
                } else if msg.contains("100001") {
                    println!("✗ 上游回 100001（请求参数错误）");
                    println!(
                        "  → 请求体字段名仍然是错的（应为 inviteCode，不能是 invitationCode）。"
                    );
                } else {
                    println!("? 未预期的错误：{msg}");
                }
            }
        }
    } else {
        println!("\n（要校验绑定请求体形状，加参数：bind-check）");
    }
}
