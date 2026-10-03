//! Loomy 账号管理器 —— 库入口。
//!
//! # 为什么分 lib.rs 与 main.rs
//!
//! Tauri 2 的约定：业务代码在 `lib.rs`（可被单元测试、可被移动端复用），
//! `main.rs` 只是薄薄的启动壳。这样 `cargo test` 能直接测业务逻辑，
//! 不必启动 GUI。
//!
//! # 架构
//!
//! ```text
//! commands/    Tauri IPC 入口（前端 invoke 的目标）
//! core/        路径、账号模型、错误、工具 —— 不认识 Tauri
//! session/     登录态三处写入（含 LevelDB）
//! clients/     上游 HTTP（积分网关）
//! ```

pub mod clients;
pub mod commands;
pub mod core;
pub mod session;

use tauri::Manager;

/// 按显示器可用空间算出窗口尺寸（逻辑 px）。
///
/// 抽成纯函数是为了**能测**：把"窗口比屏幕还宽"这个 bug 用一条测试钉住，
/// 不必真的去改系统缩放。
///
/// - `monitor`：显示器物理尺寸
/// - `scale`：Windows 显示缩放（1.25 = 125%）
pub fn fit_window_size(monitor_w: u32, monitor_h: u32, scale: f64) -> (f64, f64) {
    const WANT_W: f64 = 1400.0;
    const WANT_H: f64 = 860.0;
    /// 留出窗口边框与任务栏的余量
    const MARGIN: f64 = 60.0;
    /// 下限（低于这个值界面就没法用了）
    const MIN_W: f64 = 900.0;
    const MIN_H: f64 = 600.0;

    if scale <= 0.0 {
        return (WANT_W, WANT_H);
    }
    // 物理 → 逻辑：除以缩放比例
    let avail_w = monitor_w as f64 / scale - MARGIN;
    let avail_h = monitor_h as f64 / scale - MARGIN;
    if avail_w <= 0.0 || avail_h <= 0.0 {
        return (WANT_W, WANT_H);
    }

    (
        WANT_W.min(avail_w).max(MIN_W),
        WANT_H.min(avail_h).max(MIN_H),
    )
}

/// 把窗口调整到**当前显示器放得下**的尺寸。
///
/// # 为什么不能只在 tauri.conf.json 里写死尺寸
///
/// 实测踩到的坑：配置文件写 `1400x860`（逻辑 px），本机显示器是
/// 1728x1104 物理、**Windows 缩放 125%** —— 逻辑可用宽只有
/// `1728 / 1.25 = 1382`。于是窗口按 1400 逻辑宽创建（= 1750 物理），
/// **比屏幕还宽 22px**，右边缘被切到屏幕外。
///
/// 现象看起来像"页面右侧被截断"，很容易误判成 CSS 溢出。
///（无头浏览器的测量结果是 4 个页面 × 3 个宽度全部零溢出 —— 布局没问题，
/// 是窗口本身超界。）
///
/// 用户的显示器尺寸与缩放比例千差万别，硬编码任何数值都会在某些机器上
/// 超出。所以这里按实际工作区算。
fn fit_window_to_monitor(w: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = w.current_monitor() else {
        return; // 拿不到显示器信息就保持配置文件里的尺寸
    };
    let (wanted_w, wanted_h) = fit_window_size(
        monitor.size().width,
        monitor.size().height,
        monitor.scale_factor(),
    );
    let _ = w.set_size(tauri::LogicalSize::new(wanted_w, wanted_h));
    // 尺寸变了，重新居中
    let _ = w.center();
}

/// 构建并运行应用。
///
/// 放在 lib 里（而不是 main.rs 内联）：这样集成测试也能构造 App
/// （Tauri 提供了 `tauri::test` 用于无头测试）。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例：再次启动时把已有窗口拉到前面，而不是开第二个。
        //
        // 为什么必须：两个实例同时切账号会互相覆盖文件，
        // 而"谁赢了"取决于时序 —— 那种 bug 极难排查。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                fit_window_to_monitor(&w);
                // 窗口在 tauri.conf.json 里是 visible:false —— 这里再显示，
                // 避免白屏一闪（先渲染好再露出来）。
                let _ = w.show();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 账号
            commands::account_cmd::list_accounts,
            commands::account_cmd::active_account,
            commands::account_cmd::import_text,
            commands::account_cmd::import_dir,
            commands::account_cmd::delete_account,
            commands::account_cmd::update_account_note,
            commands::account_cmd::export_account,
            // 切换
            commands::switch_cmd::precheck_switch,
            commands::switch_cmd::switch_account,
            commands::switch_cmd::restore_backup,
            commands::switch_cmd::list_backups,
            commands::switch_cmd::kill_loomy,
            commands::switch_cmd::launch_loomy,
            // 元数据（余额 / 任务 / 邀请码）
            commands::meta_cmd::fetch_balance,
            commands::meta_cmd::fetch_balances,
            commands::meta_cmd::fetch_tasks,
            commands::meta_cmd::complete_all_tasks,
            commands::meta_cmd::fetch_activation,
            commands::meta_cmd::bind_invite,
            commands::meta_cmd::fetch_my_invites,
            commands::meta_cmd::claim_first_login,
            // 系统状态（真实路径 / 手动指定）
            commands::system_cmd::system_status,
            commands::system_cmd::set_loomy_path,
            commands::system_cmd::clear_loomy_path,
            commands::system_cmd::test_loomy_path,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Loomy 账号管理器失败");
}

#[cfg(test)]
mod tests {
    use super::fit_window_size;

    /// 默认情况：屏幕够大 → 用偏好尺寸。
    #[test]
    fn uses_preferred_size_when_screen_is_big_enough() {
        // 2560x1440 物理 @100%
        assert_eq!(fit_window_size(2560, 1440, 1.0), (1400.0, 860.0));
    }

    /// **这条钉住实测踩到的 bug**：1728x1104 物理 @125% 时逻辑可用宽
    /// 只有 1382，窗口必须是 1382-60=1322，**不能**是 1400 ——
    /// 否则窗口比屏幕还宽，右边缘被切到屏幕外。
    #[test]
    fn shrinks_to_fit_high_dpi_screen() {
        let (w, h) = fit_window_size(1728, 1104, 1.25);
        assert!(
            w < 1400.0,
            "125% 缩放下 1400 逻辑宽会超出屏幕，必须缩小（得到 {w}）"
        );
        assert_eq!(w, 1728.0 / 1.25 - 60.0, "应当正好用满可用宽度减余量");
        assert!(h <= 860.0);
        // 换算成物理像素后不能超界
        assert!(w * 1.25 <= 1728.0, "换算回物理像素仍不能超过 1728");
    }

    /// 极小屏幕：不能缩到比下限还小（否则界面没法用）。
    #[test]
    fn never_goes_below_minimum() {
        let (w, h) = fit_window_size(800, 600, 1.0);
        assert_eq!(w, 900.0);
        assert_eq!(h, 600.0);
    }

    /// 缩放比例异常（0 或负）时退回偏好尺寸，不该 panic 或产生 NaN。
    #[test]
    fn handles_bad_scale_gracefully() {
        assert_eq!(fit_window_size(1920, 1080, 0.0), (1400.0, 860.0));
        assert_eq!(fit_window_size(1920, 1080, -1.0), (1400.0, 860.0));
        let (w, h) = fit_window_size(1920, 1080, 3.0);
        assert!(w.is_finite() && h.is_finite());
    }

    /// 各种常见分辨率 × 缩放组合下，窗口都必须放得下。
    ///
    /// 这条是"通用性"检查：换台机器也不该再出现"右边被切"。
    #[test]
    fn fits_every_common_screen() {
        for (mw, mh, scale, label) in [
            (1920u32, 1080u32, 1.0f64, "1080p @100%"),
            (1920, 1080, 1.25, "1080p @125%"),
            (1920, 1080, 1.5, "1080p @150%"),
            (2560, 1440, 1.0, "1440p @100%"),
            (2560, 1440, 1.5, "1440p @150%"),
            (3840, 2160, 1.5, "4K @150%"),
            (3840, 2160, 2.0, "4K @200%"),
            (1728, 1104, 1.25, "本机（缩放 125%）"),
            (1366, 768, 1.0, "笔记本 1366x768"),
            (2880, 1800, 2.0, "MacBook Retina"),
        ] {
            let (w, h) = fit_window_size(mw, mh, scale);
            assert!(
                w * scale <= mw as f64,
                "{label}: 窗口宽 {w} 逻辑 × {scale} = {} 物理 > 屏幕 {mw} —— 会被切",
                w * scale
            );
            assert!(
                h * scale <= mh as f64,
                "{label}: 窗口高 {h} 逻辑 × {scale} = {} 物理 > 屏幕 {mh}",
                h * scale
            );
            assert!(w >= 900.0 && h >= 600.0, "{label}: 缩得太小 {w}x{h}");
        }
    }
}
