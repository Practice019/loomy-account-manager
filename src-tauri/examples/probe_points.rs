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
}
