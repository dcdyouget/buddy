// Windows 平台特定代码：绕过前台窗口锁定 (Foreground Lock)

use tauri::WebviewWindow;

/// 显示窗口并强制将其带到前台，绕过 Windows 的前台窗口锁定
///
/// ## 为什么需要这个
///
/// Windows 的前台窗口锁定会阻止"非前台进程"的窗口抢到焦点（`SetForegroundWindow`）。
/// 后果是：全局快捷键触发时，`show()` + `set_focus()` 的简单组合可能让窗口"显示了但失焦"，
/// 紧接着 `WindowEvent::Focused(false)` 事件触发，200ms 冷却一过窗口就被自动隐藏回去。
/// 用户体验就是"按一次快捷键没反应，要按两次"。
///
/// 全局快捷键（`RegisterHotKey`）会临时授予进程前台权限，但这个权限有时效性，
/// 单靠 `set_focus()` 抢不到稳定的焦点。
///
/// ## 实现原理
///
/// 关键技巧是 `always_on_top` 的 toggle（`false → true`）：
/// - `set_always_on_top(false)`：让窗口退出 always-on-top 层
/// - `set_always_on_top(true)`：让窗口重新加入 always-on-top 层（此时排在最顶）
/// - DWM 会强制重新合成 z 序，配合热键给我们的前台权限，窗口就能稳定拿到焦点
///
/// 这是 Tauri 社区广泛使用的方案（参见 tauri-plugin-global-shortcut 的 issue 讨论），
/// 不引入新的 native 依赖，纯 Tauri API 即可。
///
/// ## 配合的窗口配置
///
/// tauri.conf.json 中：
/// - `alwaysOnTop: true`：窗口已经在顶层，toggle 才能触发 z 序刷新
/// - `skipTaskbar: true`：窗口不在任务栏，DWM 单独处理
pub fn bring_to_front(window: &WebviewWindow) {
    // 1. show + unminimize：确保窗口是可见且非最小化状态
    let _ = window.show();
    let _ = window.unminimize();

    // 2. toggle always_on_top：触发 DWM 重新计算 z 序，绕过前台锁定
    //    因为窗口配置里 alwaysOnTop: true，所以最终的 always_on_top 状态不变
    let _ = window.set_always_on_top(false);
    let _ = window.set_always_on_top(true);

    // 3. set_focus：在 z 序刷新后调用，这次能稳定拿到焦点
    let _ = window.set_focus();
}