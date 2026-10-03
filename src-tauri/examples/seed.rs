//! 开发工具：把一个目录里的账号导入库里。
//!
//! ```bash
//! cargo run --example seed -- "D:\path\to\auths\loomy"
//! ```
//!
//! 不传参数时打印用法并退出 —— **不设默认路径**。第一版默认指向我自己的
//! 网关目录，那是机器相关的东西，不该留在分发的仓库里。
//!
//! 导入逻辑与 `import_dir` 命令共用 `core::account::import_from_dir`，
//! 所以这里也顺带验证了真实文件的解析。

use loomy_account_manager_lib::core::{account, paths};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = args.first() else {
        eprintln!("用法: cargo run --example seed -- <含账号 JSON 的目录>");
        eprintln!();
        eprintln!("目录可以是：");
        eprintln!("  - 含 *.json 的目录");
        eprintln!("  - 含 auths/<上游>/*.json 的目录（网关的布局）");
        std::process::exit(2);
    };

    println!("库文件: {}", account::store_file().display());
    println!("导入目录: {dir}");

    let p = std::path::PathBuf::from(dir);
    if !p.is_dir() {
        eprintln!("目录不存在: {dir}");
        std::process::exit(1);
    }

    let (found, skipped) = account::import_from_dir(&p);
    println!("扫描到 {} 个（跳过 {} 个）", found.len(), skipped.len());
    for s in &skipped {
        println!("  跳过: {s}");
    }
    if found.is_empty() {
        eprintln!("没找到任何账号。确认目录里有带 session 与 userid/uid 的 JSON。");
        std::process::exit(1);
    }

    let mut list = account::load().unwrap_or_default();
    println!("导入前库里有 {} 个", list.len());
    let mut added = 0;
    let mut updated = 0;
    for a in found {
        if account::upsert(&mut list, a) {
            added += 1;
        } else {
            updated += 1;
        }
    }
    account::save(&list).unwrap();
    println!("新增 {added}，更新 {updated}，现在共 {} 个", list.len());
    println!("数据目录: {}", paths::data_dir().display());
}
