// 全局热键管理模块：注册、注销和更新全局快捷键
//
// 提供 HotkeyState 状态管理以及 register / unregister / update_hotkey 三个核心函数。
// 当用户修改热键配置后，update_hotkey 负责先注销旧热键再注册新热键，保证始终只有一个有效热键。

use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 当前注册的全局热键状态
///
/// 使用 Mutex 包裹以支持跨线程安全访问。
/// 当用户修改热键配置时，update_hotkey 先从此状态读取旧热键并注销，再注册新热键并更新此状态。
pub struct HotkeyState {
    pub current: std::sync::Mutex<Option<Shortcut>>,
}

/// 注册全局热键并绑定窗口 toggle 行为
///
/// 按下热键时：
/// - 若主窗口当前可见 → 隐藏窗口
/// - 若主窗口当前不可见 → 获取选中文本（macOS 下通过 Cmd+C 模拟）、显示窗口、聚焦窗口
///
/// 仅在按键按下（Pressed）时触发，避免按下/释放各触发一次。
pub fn register(app: &tauri::AppHandle, shortcut: Shortcut) {
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _sc, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(window) = app.get_webview_window("main") {
                    let was_visible = window.is_visible().unwrap_or(false);
                    if was_visible {
                        let _ = window.hide();
                    } else {
                        crate::get_selected_text(app);
                        // 多屏适配：将窗口移到光标所在的显示器中心
                        crate::reposition_to_cursor_monitor(&window);
                        // 等待一帧（~16ms），确保窗口服务器已应用新的 frame 再 show，
                        // 避免跨屏时窗口在旧位置短暂闪现
                        std::thread::sleep(std::time::Duration::from_millis(16));
                        let _ = window.show();
                        let _ = window.set_focus();
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
        let mut current = state.current.lock().unwrap();
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
        .unwrap()
        .replace(shortcut);
}
