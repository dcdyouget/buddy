// 全局热键管理模块：注册、注销和更新全局快捷键
//
// 提供 HotkeyState 状态管理以及 register / unregister / update_hotkey 三个核心函数。
// 当用户修改热键配置后，update_hotkey 负责先注销旧热键再注册新热键，保证始终只有一个有效热键。

use log;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 当前注册的全局热键状态
///
/// 使用 Mutex 包裹以支持跨线程安全访问。
/// 当用户修改热键配置时，update_hotkey 先从此状态读取旧热键并注销，再注册新热键并更新此状态。
pub struct HotkeyState {
    pub current: parking_lot::Mutex<Option<Shortcut>>,
}

/// 注册全局热键并绑定窗口 toggle 行为
///
/// 三态切换（参考 Bob 等 macOS 工具型应用的标准行为）：
/// - **隐藏状态** → 获取选中文本、重新定位到当前光标所在显示器、显示、聚焦
/// - **显示但失焦**（用户点到了别的窗口，窗口在 z 序后面）→ 唤回到最前，不改位置
/// - **显示且已聚焦**（在最前）→ 隐藏
///
/// 仅在按键按下（Pressed）时触发，避免按下/释放各触发一次。
pub fn register(app: &tauri::AppHandle, shortcut: Shortcut) {
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _sc, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    let focused = window.is_focused().unwrap_or(false);
                    log::info!("[hotkey] pressed, visible={}, focused={}", visible, focused);

                    if visible && focused {
                        // ── 显示 + 已聚焦 → 隐藏 ──
                        let _ = window.hide();
                        log::info!("[hotkey] hidden");
                    } else {
                        // ── 隐藏 OR 显示但失焦 → 带到最前 ──
                        // 仅在从"隐藏"唤起时才重新定位到当前光标所在显示器；
                        // 失焦唤回不重定位（保留用户拖拽后的位置）。
                        if !visible {
                            crate::platform::macos::get_selected_text(app);
                            log::info!("[hotkey] before reposition (hidden → show)");
                            crate::window::positioning::reposition_to_cursor_monitor(&window);
                            log::info!("[hotkey] before sleep");
                            std::thread::sleep(std::time::Duration::from_millis(16));
                        } else {
                            log::info!("[hotkey] shown+unfocused → bring_to_front (no reposition)");
                        }

                        log::info!("[hotkey] before bring_to_front, pos={:?}", window.outer_position().ok());

                        // Windows 上需要绕过前台窗口锁定；其他平台直接 show + set_focus
                        #[cfg(target_os = "windows")]
                        crate::platform::windows::bring_to_front(&window);
                        #[cfg(not(target_os = "windows"))]
                        {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }

                        log::info!("[hotkey] after bring_to_front, visible={}, focused={}",
                            window.is_visible().unwrap_or(false),
                            window.is_focused().unwrap_or(false));
                    }
                }
            }
        })
        .ok();
}

/// 注销指定的全局热键
///
/// 从全局快捷键管理器中移除指定热键的注册。
/// 若热键未注册或注销失败，静默忽略错误。
pub fn unregister(app: &tauri::AppHandle, shortcut: Shortcut) {
    let _ = app.global_shortcut().unregister(shortcut);
}

/// 更新全局热键：注销旧热键 → 注册新热键 → 更新状态
///
/// 参数 `hotkey_str` 为用户配置的热键字符串（如 "CmdOrCtrl+J"）。
/// 若字符串解析失败（无效热键格式），静默忽略，不改变当前注册的热键。
pub fn update_hotkey(app: &tauri::AppHandle, hotkey_str: &str) {
    let shortcut = match hotkey_str.parse::<Shortcut>() {
        Ok(s) => s,
        Err(_) => return,
    };

    // 先注销旧热键
    {
        let state = app.state::<HotkeyState>();
        let mut current = state.current.lock();
        if let Some(old) = current.take() {
            unregister(app, old);
        }
    }

    // 注册新热键
    register(app, shortcut);

    // 更新状态
    app.state::<HotkeyState>()
        .current
        .lock()
        .replace(shortcut);
}
