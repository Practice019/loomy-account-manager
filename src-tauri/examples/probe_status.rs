//! 开发工具：打印 `system_status` 的返回，验证设置页拿到的是**真实**路径。
//!
//! # 为什么要有这个
//!
//! 设置页原来把 `%APPDATA%\LoomyAccountManager` 这个字符串**写死在前端**
//! 当作"数据目录"显示 —— 那跟后端毫无关系。如果实际目录不是它
//!（换了环境变量，或走了兜底分支），界面会显示一个错误位置。
//!
//! 这条探针打印后端**真正在用**的路径。与界面显示的值对照，即可确认
//! 前端没有自己编造。
//!
//! 用法：cargo run --example probe_status

use loomy_account_manager_lib::commands::system_cmd::system_status;

fn main() {
    let s = system_status();

    println!("═══ system_status（设置页显示的就是这些）═══");
    println!("探测到 Loomy : {}", s.found);
    println!("安装根       : {}", s.root.as_deref().unwrap_or("(未找到)"));
    println!(
        "登录态文件   : {}",
        s.auth_file.as_deref().unwrap_or("(未找到)")
    );
    println!(
        "leveldb 目录 : {}",
        if s.leveldb_dirs.is_empty() {
            "(未找到)".into()
        } else {
            s.leveldb_dirs.join(", ")
        }
    );
    println!(
        "leveldb 日志 : {}",
        s.leveldb_log.as_deref().unwrap_or("(未找到)")
    );
    println!("Loomy.exe    : {}", s.exe.as_deref().unwrap_or("(未找到)"));
    println!("运行中 PID   : {:?}", s.running_pids);
    println!("数据目录     : {}", s.data_dir);
    println!("账号库       : {}", s.accounts_file);
    println!(
        "手动指定     : {}",
        s.manual_path.as_deref().unwrap_or("(无，用自动探测)")
    );
    println!("需要手动指定 : {}", s.needs_manual);
    if !s.reason.is_empty() {
        println!("原因         : {}", s.reason);
    }
    if !s.guess_candidates.is_empty() {
        println!("候选目录     :");
        for c in &s.guess_candidates {
            println!("    {c}");
        }
    }

    // ── 断言：数据目录必须是真实的、可写的 ──────────────────────────────
    println!();
    let data = std::path::Path::new(&s.data_dir);
    assert!(data.is_dir(), "数据目录必须真实存在: {}", s.data_dir);
    println!("✓ 数据目录真实存在: {}", data.display());

    // 账号库的父目录必须就是数据目录（不能是前端编的字符串）
    let store = std::path::Path::new(&s.accounts_file);
    assert_eq!(
        store.parent().map(|p| p.to_path_buf()),
        Some(data.to_path_buf()),
        "账号库必须就在数据目录下"
    );
    println!("✓ 账号库在数据目录下: {}", store.display());

    // exe 若返回，必须是真实文件（防止硬编码别的机器）
    if let Some(exe) = &s.exe {
        assert!(
            std::path::Path::new(exe).is_file(),
            "返回的 exe 必须真实存在（否则就是硬编码）: {exe}"
        );
        println!("✓ Loomy.exe 真实存在: {exe}");
    }

    // 探测逻辑自洽性检查
    if s.needs_manual {
        assert!(
            !s.reason.is_empty(),
            "需要手动指定时必须给出原因（否则用户不知道要干什么）"
        );
        println!("✓ 需要手动指定，且给出了原因");
    }
    if s.found {
        assert!(
            s.root.is_some() && s.auth_file.is_some(),
            "找到了就必须给出安装根与登录态文件"
        );
        println!("✓ 找到安装时给出了完整路径");
    }

    println!("\n全部检查通过");
}
