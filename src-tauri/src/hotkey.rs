// 全局热键管理模块：注册、注销和更新全局快捷键
//
// 提供 HotkeyState 状态管理以及 register / unregister / update_hotkey 三个核心函数。
// 当用户修改热键配置后，update_hotkey 负责先注销旧热键再注册新热键，保证始终只有一个有效热键。

use log;
use tauri::{Emitter, Manager};
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
/// - **隐藏状态** → 获取选中文本、定位到鼠标所在屏幕、显示、聚焦
/// - **显示但失焦**（用户点到了别的窗口，窗口在 z 序后面）→ 定位并唤回到最前
/// - **显示且已聚焦**（在最前）→ 隐藏
///
/// 仅在按键按下（Pressed）时触发，避免按下/释放各触发一次。
///
/// 返回 Result：注册失败（组合被系统保留、被其它应用占用等）时向上传播，
/// 调用方才能把错误反馈给用户，而不是静默吞掉。
pub fn register(app: &tauri::AppHandle, shortcut: Shortcut) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _sc, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    let focused = window.is_focused().unwrap_or(false);
                    log::info!("[hotkey] pressed, visible={}, focused={}", visible, focused);

                    if visible && focused {
                        // ── 显示 + 已聚焦 → 隐藏 ──
                        let _ = window.emit(crate::window::WINDOW_WILL_HIDE_EVENT, ());
                        if let Err(error) = window.hide() {
                            crate::window::emit_window_will_show(&window, false);
                            log::warn!("[hotkey] hide failed: {error}");
                        } else {
                            log::info!("[hotkey] hidden");
                        }
                    } else {
                        // ── 隐藏 OR 显示但失焦 → 带到鼠标所在屏幕的前台 ──
                        if !visible {
                            crate::platform::macos::get_selected_text(app);
                        } else {
                            log::info!("[hotkey] shown+unfocused → bring_to_front");
                        }

                        crate::window::positioning::reposition_to_cursor_monitor(&window);

                        // 先完成跨屏定位，再让前端计算气泡尺寸，避免异步缩放把窗口带回旧屏幕。
                        crate::window::notify_window_invoked(&window, true);
                        log::info!(
                            "[hotkey] before bring_to_front, pos={:?}",
                            window.outer_position().ok()
                        );

                        // Windows/macOS 都需要平台专用的前台激活逻辑。
                        #[cfg(target_os = "windows")]
                        crate::platform::windows::bring_to_front(&window);
                        #[cfg(target_os = "macos")]
                        crate::platform::macos::bring_to_front(&window);
                        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                        {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }

                        log::info!(
                            "[hotkey] after bring_to_front, visible={}, focused={}",
                            window.is_visible().unwrap_or(false),
                            window.is_focused().unwrap_or(false)
                        );
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|error| format!("全局快捷键注册失败: {}", error))
}

/// 注销指定的全局热键
///
/// 从全局快捷键管理器中移除指定热键的注册。
/// 若热键未注册或注销失败，静默忽略错误。
pub fn unregister(app: &tauri::AppHandle, shortcut: Shortcut) {
    let _ = app.global_shortcut().unregister(shortcut);
}

/// 更新全局热键：注册新热键 → 注销旧热键 → 更新状态
///
/// 参数 `hotkey_str` 为用户配置的热键字符串（如 "CmdOrCtrl+J"）。
///
/// 顺序设计（关键安全点）：
/// 1. 先注册新热键；若注册失败（组合被系统保留/被其它应用占用）直接返回 Err，
///    **旧热键保持可用**，不会出现"旧键已注销、新键注册失败"的静默失能。
/// 2. 注册成功后，才注销旧热键并更新状态。
/// 3. 若新组合与当前组合相同，直接返回（避免每次保存配置都注销/注册一遍全局快捷键）。
pub fn update_hotkey(app: &tauri::AppHandle, hotkey_str: &str) -> Result<(), String> {
    let shortcut = hotkey_str
        .parse::<Shortcut>()
        .map_err(|_| format!("无效的热键格式: {}", hotkey_str))?;

    let state = app.state::<HotkeyState>();
    let mut current = state.current.lock();
    // 组合未变化：无需重新注册（正常保存配置时 hotkey 大多没变）
    if current.as_ref() == Some(&shortcut) {
        return Ok(());
    }

    // 先注册新热键，成功后再注销旧热键
    register(app, shortcut)?;

    if let Some(old) = current.replace(shortcut) {
        unregister(app, old);
    }
    Ok(())
}
