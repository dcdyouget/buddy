// 多屏窗口定位模块

use super::geometry::{
    calculate_bottom_anchored_target_geometry, physical_page_size, physical_window_margin,
    WindowPosition, WindowSize, WorkArea,
};
use log::{info, warn};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use tauri::Manager;

const SLOW_WINDOW_OPERATION: Duration = Duration::from_millis(50);

/// 记录用户拖拽后的窗口位置（物理像素），key 为显示器 (x,y)，value 为窗口 (x,y)
#[derive(Default)]
pub struct SavedWindowPositions {
    pub positions: Mutex<HashMap<(i32, i32), (i32, i32)>>,
    pub pending_move_save: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub move_generation: AtomicU64,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct FocusedScreenHint {
    name: String,
    frontmost_app: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// 快捷键呼出时定位到当前接收键盘事件的窗口所在显示器。
/// macOS 的 NSScreen.mainScreen 正是“当前键盘焦点窗口所在屏幕”，不会受鼠标位置影响。
#[cfg(target_os = "macos")]
pub fn reposition_to_focused_monitor(window: &tauri::WebviewWindow, trace_id: u64) {
    let started_at = Instant::now();
    let lookup_started_at = Instant::now();
    let focused = macos_focused_screen_hint(window);
    let focused_monitor = focused
        .as_ref()
        .and_then(|hint| match_focused_screen_monitor(window, hint));
    let lookup_elapsed = lookup_started_at.elapsed();

    if let Some(monitor) = focused_monitor {
        let context = focused
            .as_ref()
            .map(|hint| {
                format!(
                    "source=focused-window, app={}, screen={}, appkit_frame=({:.0},{:.0},{:.0}x{:.0})",
                    hint.frontmost_app, hint.name, hint.x, hint.y, hint.width, hint.height,
                )
            })
            .unwrap_or_else(|| "source=focused-window".to_string());
        reposition_to_monitor(
            window,
            monitor,
            started_at,
            lookup_elapsed,
            Some(trace_id),
            &context,
        );
        return;
    }

    let fallback_context = match focused {
        Some(hint) => format!(
            "source=cursor-fallback, app={}, unmatched_screen={}, appkit_frame=({:.0},{:.0},{:.0}x{:.0})",
            hint.frontmost_app, hint.name, hint.x, hint.y, hint.width, hint.height,
        ),
        None => "source=cursor-fallback, focused_screen=unavailable".to_string(),
    };
    if let Some(monitor) = cursor_monitor(window) {
        reposition_to_monitor(
            window,
            monitor,
            started_at,
            lookup_elapsed,
            Some(trace_id),
            &fallback_context,
        );
    } else {
        super::log_diagnostic(&format!(
            "[window-diag] id={trace_id} reposition failed, {fallback_context}, total={}ms",
            started_at.elapsed().as_millis(),
        ));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn reposition_to_focused_monitor(window: &tauri::WebviewWindow, _trace_id: u64) {
    reposition_to_cursor_monitor(window);
}

/// 托盘点击仍跟随鼠标所在显示器；用户点击的菜单栏位置就是明确目标。
pub fn reposition_to_cursor_monitor(window: &tauri::WebviewWindow) {
    let started_at = Instant::now();
    let lookup_started_at = Instant::now();
    let Some(monitor) = cursor_monitor(window) else {
        return;
    };
    let lookup_elapsed = lookup_started_at.elapsed();
    reposition_to_monitor(
        window,
        monitor,
        started_at,
        lookup_elapsed,
        None,
        "source=cursor",
    );
}

fn cursor_monitor(window: &tauri::WebviewWindow) -> Option<tauri::window::Monitor> {
    let cursor_pos = window.cursor_position().ok()?;
    match window.monitor_from_point(cursor_pos.x, cursor_pos.y) {
        Ok(Some(monitor)) => Some(monitor),
        _ => window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.primary_monitor().ok().flatten()),
    }
}

#[cfg(target_os = "macos")]
fn macos_focused_screen_hint(window: &tauri::WebviewWindow) -> Option<FocusedScreenHint> {
    use objc2::MainThreadMarker;

    if let Some(marker) = MainThreadMarker::new() {
        return macos_focused_screen_hint_on_main(marker);
    }

    // 全局快捷键插件通常在主线程回调；如果运行时实现发生变化，则同步转交主线程，
    // 避免因为线程不符静默退回鼠标屏幕。
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let hint = MainThreadMarker::new().and_then(macos_focused_screen_hint_on_main);
            let _ = sender.send(hint);
        })
        .ok()?;
    receiver
        .recv_timeout(Duration::from_millis(100))
        .ok()
        .flatten()
}

#[cfg(target_os = "macos")]
fn macos_focused_screen_hint_on_main(marker: objc2::MainThreadMarker) -> Option<FocusedScreenHint> {
    use objc2_app_kit::{NSScreen, NSWorkspace};

    let screen = NSScreen::mainScreen(marker)?;
    let frame = screen.frame();
    let frontmost_app = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .and_then(|application| application.bundleIdentifier())
        .map(|identifier| identifier.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Some(FocusedScreenHint {
        name: screen.localizedName().to_string(),
        frontmost_app,
        x: frame.origin.x,
        y: frame.origin.y,
        width: frame.size.width,
        height: frame.size.height,
    })
}

#[cfg(target_os = "macos")]
fn match_focused_screen_monitor(
    window: &tauri::WebviewWindow,
    focused: &FocusedScreenHint,
) -> Option<tauri::window::Monitor> {
    let primary_height = window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| monitor.size().height as f64 / monitor.scale_factor())?;
    let focused_top = primary_height - focused.y - focused.height;
    let monitors = window.available_monitors().ok()?;

    monitors.into_iter().min_by(|left, right| {
        let score = |monitor: &tauri::window::Monitor| {
            let scale = monitor.scale_factor();
            let position = monitor.position();
            let size = monitor.size();
            focused_monitor_match_score(
                monitor.name().map(String::as_str),
                position.x as f64 / scale,
                position.y as f64 / scale,
                size.width as f64 / scale,
                size.height as f64 / scale,
                focused,
                focused_top,
            )
        };
        score(left).total_cmp(&score(right))
    })
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
fn focused_monitor_match_score(
    monitor_name: Option<&str>,
    logical_x: f64,
    logical_y: f64,
    logical_width: f64,
    logical_height: f64,
    focused: &FocusedScreenHint,
    focused_top: f64,
) -> f64 {
    let geometry_score = (logical_x - focused.x).abs()
        + (logical_y - focused_top).abs()
        + (logical_width - focused.width).abs()
        + (logical_height - focused.height).abs();
    let name_penalty = if monitor_name == Some(focused.name.as_str()) {
        0.0
    } else {
        1_000_000.0
    };
    name_penalty + geometry_score
}

fn reposition_to_monitor(
    window: &tauri::WebviewWindow,
    monitor: tauri::window::Monitor,
    started_at: Instant,
    monitor_lookup_elapsed: Duration,
    trace_id: Option<u64>,
    context: &str,
) {
    let m_pos = monitor.position();
    let m_size = monitor.size();
    let w_size = match window.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let scale = monitor.scale_factor();

    let m_x = m_pos.x as f64 / scale;
    let m_y = m_pos.y as f64 / scale;
    let m_w = m_size.width as f64 / scale;
    let m_h = m_size.height as f64 / scale;
    let w_w = w_size.width as f64 / scale;
    let w_h = w_size.height as f64 / scale;

    let mut x = m_x + (m_w - w_w) / 2.0;
    let mut y = m_y + (m_h - w_h) / 2.0;
    let mut used_saved_position = false;

    // 检查是否有用户记住的位置
    {
        let saved = window
            .app_handle()
            .try_state::<SavedWindowPositions>()
            .and_then(|state| state.positions.lock().get(&(m_pos.x, m_pos.y)).copied());
        if let Some((sx, sy)) = saved {
            let sxf = sx as f64 / scale;
            let syf = sy as f64 / scale;
            if sxf >= m_x && sxf + w_w <= m_x + m_w && syf >= m_y && syf + w_h <= m_y + m_h {
                x = sxf;
                y = syf;
                used_saved_position = true;
                info!(
                    "[reposition] using saved position: ({:.0},{:.0}) logical",
                    x, y
                );
            }
        }
    }

    #[cfg(target_os = "macos")]
    let macos_y = {
        let primary_logical_h = window
            .primary_monitor()
            .ok()
            .flatten()
            .map(|m| m.size().height as f64 / m.scale_factor())
            .unwrap_or(m_y + m_h);
        primary_logical_h - y - w_h
    };
    #[cfg(not(target_os = "macos"))]
    let macos_y = 0.0_f64;

    info!(
        "[reposition] final logical pos=({:.0},{:.0}) macosY={:.0}, win_logical=({:.0}x{:.0})",
        x, y, macos_y, w_w, w_h
    );

    // macOS：使用 NSWindow.setFrameOrigin 同步设置位置
    #[cfg(target_os = "macos")]
    {
        if let Ok(ns_window_ptr) = window.ns_window() {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;
            use objc2_foundation::NSPoint;
            unsafe {
                let ns_window: *mut AnyObject = ns_window_ptr as *mut _;
                let point = NSPoint::new(x, macos_y);
                let _: () = msg_send![ns_window, setFrameOrigin: point];
            }
            log_reposition_diagnostic(
                trace_id,
                context,
                &monitor,
                x,
                y,
                used_saved_position,
                started_at.elapsed(),
                monitor_lookup_elapsed,
            );
            return;
        }
    }

    let _ = window.set_position(tauri::PhysicalPosition::new(
        (x * scale) as i32,
        (y * scale) as i32,
    ));
    log_reposition_diagnostic(
        trace_id,
        context,
        &monitor,
        x,
        y,
        used_saved_position,
        started_at.elapsed(),
        monitor_lookup_elapsed,
    );
}

#[allow(clippy::too_many_arguments)]
fn log_reposition_diagnostic(
    trace_id: Option<u64>,
    context: &str,
    monitor: &tauri::window::Monitor,
    x: f64,
    y: f64,
    used_saved_position: bool,
    total: Duration,
    monitor_lookup: Duration,
) {
    let position = monitor.position();
    let size = monitor.size();
    super::log_diagnostic(&format!(
        "[window-diag] id={} reposition total={}ms, lookup={}ms, {}, target={} frame=({},{},{}x{}) scale={:.2}, final_logical=({:.0},{:.0}), saved={used_saved_position}",
        trace_id.map_or_else(|| "tray".to_string(), |id| id.to_string()),
        total.as_millis(),
        monitor_lookup.as_millis(),
        context,
        monitor.name().map(String::as_str).unwrap_or("unknown"),
        position.x,
        position.y,
        size.width,
        size.height,
        monitor.scale_factor(),
        x,
        y,
    ));
}

/// 在 Rust 内一次完成窗口查询、目标几何计算和原生设置，避免前端多次 IPC 往返。
pub fn resize_window_to_page(window: &tauri::WebviewWindow, page: &str) -> Result<(), String> {
    let started_at = Instant::now();
    let query_started_at = Instant::now();
    let start_position = window
        .outer_position()
        .map_err(|error| format!("读取窗口位置失败：{error}"))?;
    let start_size = window
        .outer_size()
        .map_err(|error| format!("读取窗口尺寸失败：{error}"))?;
    let monitor = window
        .current_monitor()
        .map_err(|error| format!("读取当前显示器失败：{error}"))?;
    let scale_factor = match monitor.as_ref() {
        Some(monitor) => monitor.scale_factor(),
        None => window
            .scale_factor()
            .map_err(|error| format!("读取显示缩放失败：{error}"))?,
    };
    let target_size = physical_page_size(page, scale_factor)
        .ok_or_else(|| format!("不支持的窗口页面：{page}"))?;
    let work_area = monitor.as_ref().map(|monitor| {
        let work_area = monitor.work_area();
        WorkArea {
            position: WindowPosition {
                x: work_area.position.x,
                y: work_area.position.y,
            },
            size: WindowSize {
                width: work_area.size.width,
                height: work_area.size.height,
            },
        }
    });
    let query_elapsed = query_started_at.elapsed();

    let geometry = calculate_bottom_anchored_target_geometry(
        WindowPosition {
            x: start_position.x,
            y: start_position.y,
        },
        WindowSize {
            width: start_size.width,
            height: start_size.height,
        },
        target_size,
        work_area,
        physical_window_margin(scale_factor),
    );

    let apply_started_at = Instant::now();
    window
        .set_position(tauri::PhysicalPosition::new(
            geometry.position.x,
            geometry.position.y,
        ))
        .map_err(|error| format!("设置窗口位置失败：{error}"))?;
    window
        .set_size(tauri::PhysicalSize::new(
            geometry.size.width,
            geometry.size.height,
        ))
        .map_err(|error| format!("设置窗口尺寸失败：{error}"))?;
    let apply_elapsed = apply_started_at.elapsed();
    let total_elapsed = started_at.elapsed();

    let message = format!(
        "[window-perf] resize page={page}: total={}ms, query={}ms, apply={}ms, scale={scale_factor:.2}",
        total_elapsed.as_millis(),
        query_elapsed.as_millis(),
        apply_elapsed.as_millis(),
    );
    if total_elapsed >= SLOW_WINDOW_OPERATION {
        warn!("{message}");
    } else {
        info!("{message}");
    }

    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn focused_screen() -> FocusedScreenHint {
        FocusedScreenHint {
            name: "External Display".to_string(),
            frontmost_app: "com.example.editor".to_string(),
            x: 1440.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    #[test]
    fn focused_screen_geometry_selects_the_matching_duplicate_display() {
        let focused = focused_screen();
        let left = focused_monitor_match_score(
            Some("External Display"),
            0.0,
            0.0,
            1920.0,
            1080.0,
            &focused,
            0.0,
        );
        let right = focused_monitor_match_score(
            Some("External Display"),
            1440.0,
            0.0,
            1920.0,
            1080.0,
            &focused,
            0.0,
        );

        assert!(right < left);
    }

    #[test]
    fn focused_screen_name_is_preferred_over_unrelated_monitor() {
        let focused = focused_screen();
        let matching_name = focused_monitor_match_score(
            Some("External Display"),
            1430.0,
            10.0,
            1920.0,
            1080.0,
            &focused,
            0.0,
        );
        let unrelated = focused_monitor_match_score(
            Some("Built-in Display"),
            1440.0,
            0.0,
            1920.0,
            1080.0,
            &focused,
            0.0,
        );

        assert!(matching_name < unrelated);
    }
}
