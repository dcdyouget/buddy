// 多屏窗口定位模块

use log::info;
use parking_lot::Mutex;
use tauri::Manager;

/// 记录用户拖拽后的窗口位置（物理像素），key 为显示器 (x,y)，value 为窗口 (x,y)
pub struct SavedWindowPositions(pub Mutex<std::collections::HashMap<(i32, i32), (i32, i32)>>);

/// 将窗口重新定位到当前光标所在显示器的中心位置（多屏适配）
pub fn reposition_to_cursor_monitor(window: &tauri::WebviewWindow) {
    let cursor_pos = match window.cursor_position() {
        Ok(pos) => pos,
        Err(_) => return,
    };

    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(_) => return,
    };

    let target_monitor = monitors.iter().find(|m| {
        let m_pos = m.position();
        let m_size = m.size();
        cursor_pos.x >= m_pos.x as f64
            && cursor_pos.x < (m_pos.x + m_size.width as i32) as f64
            && cursor_pos.y >= m_pos.y as f64
            && cursor_pos.y < (m_pos.y + m_size.height as i32) as f64
    });

    let monitor = match target_monitor {
        Some(m) => m.clone(),
        None => match window.current_monitor() {
            Ok(Some(m)) => m,
            _ => match window.primary_monitor() {
                Ok(Some(m)) => m,
                _ => return,
            },
        },
    };

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

    // 检查是否有用户记住的位置
    {
        let saved = window
            .app_handle()
            .try_state::<SavedWindowPositions>()
            .map(|s| s.0.lock().get(&(m_pos.x, m_pos.y)).copied())
            .flatten();
        if let Some((sx, sy)) = saved {
            let sxf = sx as f64 / scale;
            let syf = sy as f64 / scale;
            if sxf >= m_x && sxf + w_w <= m_x + m_w && syf >= m_y && syf + w_h <= m_y + m_h {
                x = sxf;
                y = syf;
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
            return;
        }
    }

    let _ = window.set_position(tauri::PhysicalPosition::new(
        (x * scale) as i32,
        (y * scale) as i32,
    ));
}
