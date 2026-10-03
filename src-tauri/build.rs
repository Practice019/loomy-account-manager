//! 构建脚本。
//!
//! # 为什么除了 `tauri_build::build()` 还要多写几行
//!
//! `tauri-build` 自己只声明跟踪 `tauri.conf.json` 与 `capabilities/`：
//!
//! ```text
//! cargo:rerun-if-changed=<...>/tauri.conf.json
//! cargo:rerun-if-changed=<...>/capabilities
//! ```
//!
//! **它不跟踪 `icons/`**。后果实测过：把图标换成透明底的新版、重新
//! `cargo build`，Cargo 认为"没有任何输入变化"，直接复用上次编译出的
//! Windows 资源对象（`.res`）—— exe 里内嵌的还是**旧图标**。
//!
//! 现象很有误导性：任务栏显示旧图标，但单独打开 `icons/icon.ico` 看是
//! 完全正确的新图标（我一开始就去查 ico 文件，查了个寂寞）。更麻烦的是
//! 它**静默** —— 构建成功、没有任何警告。
//!
//! 另外 `tauri_build::build()` 失败时用 `expect` 报出来，而不是让它
//! 静默返回：资源生成失败会直接导致 exe 没有图标，那是很难从现象
//! 反推回来的（看起来只是"图标没生效"）。

fn main() {
    // 图标变了必须重新编译资源。
    // 监听整个目录（而不只是单个文件）—— 新增尺寸文件时也能覆盖到。
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");

    tauri_build::build()
}
