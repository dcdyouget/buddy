//! S00-03 产物：全局热键 / tray / autostart 与 GPUI 集成的规范化模式（macOS）
//!
//! **为什么需要这个文件**：`spikes/` 被 `.gitignore` 排除，spike 代码会丢失。
//! 本文件固化已实测可用的集成模式，后续 spec 直接取用：
//!   - `S07-03` 全局热键唤起与切换
//!   - `S07-09` tray 图标与菜单
//!   - `S07-10` 开机自启
//!
//! 实测依据见 `docs/specs/phase-00/S00-03-hotkey-tray-autostart.md` 的「证据」段。
//!
//! ## 核心结论：**三者与 GPUI 事件循环共存，不需要另起事件循环**
//!
//! GPUI 的 macOS 后端跑的是 `NSApplication` 的 run loop（即 CFRunLoop）。
//! Carbon 热键事件与 `NSMenu`/`NSStatusItem` 事件都由同一个 run loop 分发，
//! 因此**在 `application().run(..)` 闭包内直接初始化即可**。
//!
//! ## 依赖版本（**必须与 gpui 一致**）
//!
//! | crate | 版本 | objc2 要求 | 与 gpui 兼容 |
//! |-------|------|-----------|-------------|
//! | `global-hotkey` | 0.8 | `objc2 ^0.6` / `app-kit ^0.3` | ✅ |
//! | `tray-icon` | 0.24 | `objc2 ^0.6` / `app-kit ^0.3` | ✅ |
//! | `auto-launch` | 0.6 | macOS **零依赖**（只写 LaunchAgent plist） | ✅ |
//!
//! 实测：`gpui_macos` 与上述三者在 `objc2 0.6.4` / `objc2-app-kit 0.3.2` 上**共用同一版本**。
//! （树里另有 `objc2 0.5.2`，来自 gpui 自己的 `accesskit_macos`，与本集成无关。）
//!
//! `tao` / `winit` **不会进入构建树**（它们是 `tray-icon`/`global-hotkey` 的 dev-dependency）。

#![allow(dead_code)]

use std::cell::RefCell;
use std::sync::OnceLock;

// ═══════════════════════════════════════════════════════════════
// 关键约束 1：`TrayIcon` 不是 Send/Sync → 必须活在主线程
// ═══════════════════════════════════════════════════════════════
//
// `tray_icon::TrayIcon` 内部是 `Rc<RefCell<..>>`，**不能放进 `static`**
// （编译错误：`Rc<RefCell<TrayIcon>>` cannot be shared between threads safely）。
//
// 两种正确做法：
//   (a) `thread_local!` + `RefCell<Option<TrayIcon>>`（本文件采用）
//   (b) 存进 GPUI 的 `Entity<T>`（GPUI 的 Entity 本身就是主线程独占，语义更贴）
//
// 推荐 (b)：让 tray 的生存期与某个 `Entity` 绑定，避免 thread_local 的隐式泄漏。
thread_local! {
    static TRAY: RefCell<Option<tray_icon::TrayIcon>> = const { RefCell::new(None) };
}

// `GlobalHotKeyManager` 是 Send + Sync，可以放 static。
// 但**必须保持存活**（drop 即注销热键），因此用 OnceLock 持有到进程结束。
static HOTKEY_MANAGER: OnceLock<global_hotkey::GlobalHotKeyManager> = OnceLock::new();

// ═══════════════════════════════════════════════════════════════
// 1. 全局热键
// ═══════════════════════════════════════════════════════════════

/// 注册全局热键。返回注册结果描述。
///
/// ## 事件送达机制
/// `GlobalHotKeyEvent::set_event_handler` 注册的闭包**会在 GPUI 的主线程上被调用**
/// （因为 Carbon 事件由 NSApplication 的 run loop 分发）。实测：按 7 次热键，7 次回调。
///
/// ## 跨线程更新 GPUI 状态
/// 回调是 `Fn + Send + Sync + 'static`，拿不到 `Context`。
/// 需要更新 UI 时用「原子量 / channel + GPUI 侧轮询」：
/// 见 `HOTKEY_HITS` 与 `cx.spawn` 轮询的配合。
pub fn register_global_hotkey<F>(on_press: F) -> Result<String, String>
where
    F: Fn(global_hotkey::hotkey::HotKey) + Send + Sync + 'static,
{
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

    let manager = GlobalHotKeyManager::new().map_err(|e| format!("manager: {e}"))?;

    // Buddy 默认热键。注意此处用 SUPER 表示 ⌘。
    let hotkey = HotKey::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyB);
    manager.register(hotkey).map_err(|e| format!("register: {e}"))?;

    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state == HotKeyState::Pressed {
            on_press(hotkey);
        }
    }));

    HOTKEY_MANAGER
        .set(manager)
        .map_err(|_| "manager 已存在（重复注册）".to_string())?;

    Ok(format!("已注册 {hotkey}"))
}

// ═══════════════════════════════════════════════════════════════
// 2. Tray 图标 + 菜单
// ═══════════════════════════════════════════════════════════════

/// 生成纯色 RGBA 图标（避免依赖资源文件）
pub fn solid_icon(r: u8, g: u8, b: u8, size: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((size * size * 4) as usize);
    for _ in 0..size * size {
        v.extend_from_slice(&[r, g, b, 255]);
    }
    v
}

/// 创建 tray 图标与菜单。返回各菜单项 id，供事件处理分发。
pub fn create_tray() -> Result<TrayHandles, String> {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    let menu = Menu::new();
    let toggle = MenuItem::new("显示 / 隐藏", true, None);
    let settings = MenuItem::new("设置…", true, None);
    let sep = PredefinedMenuItem::separator();
    let quit = MenuItem::new("退出", true, None);

    let handles = TrayHandles {
        toggle: toggle.id().0.clone(),
        settings: settings.id().0.clone(),
        quit: quit.id().0.clone(),
    };

    for item in [&toggle, &settings] {
        menu.append(item).map_err(|e| format!("append: {e}"))?;
    }
    menu.append(&sep).map_err(|e| format!("append: {e}"))?;
    menu.append(&quit).map_err(|e| format!("append: {e}"))?;

    // 32×32 品牌色（#5B5FE9，见 AGENTS.md 硬约束 2）
    let rgba = solid_icon(0x5B, 0x5F, 0xE9, 32);
    let icon = tray_icon::Icon::from_rgba(rgba, 32, 32).map_err(|e| format!("icon: {e}"))?;

    let tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Buddy")
        .with_icon(icon)
        .build()
        .map_err(|e| format!("build: {e}"))?;

    // 必须存进主线程，否则 drop 后图标消失
    TRAY.with(|t| *t.borrow_mut() = Some(tray));

    Ok(handles)
}

/// tray 菜单项 id，用于在事件回调里分发
pub struct TrayHandles {
    pub toggle: String,
    pub settings: String,
    pub quit: String,
}

/// 注册 tray 菜单点击回调。
///
/// 与热键同理：回调在主线程被调用，但拿不到 `Context`。
///
/// ## ⚠️ 已验证：tray 不会抢占 `NSApp` 的 delegate
/// 实测 `[NSApp delegate]` 在 tray 创建前后**都是 `GPUIApplicationDelegate`**。
/// 这是本集成最大的风险点（若 `tray-icon` 调 `setDelegate:` 会让 GPUI 事件系统失效），
/// 实测未发生。
pub fn set_tray_menu_handler<F>(handles: TrayHandles, on_menu: F)
where
    F: Fn(&'static str) + Send + Sync + 'static,
{
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |e: tray_icon::menu::MenuEvent| {
        let which = if e.id.0 == handles.toggle {
            "toggle"
        } else if e.id.0 == handles.settings {
            "settings"
        } else if e.id.0 == handles.quit {
            "quit"
        } else {
            "unknown"
        };
        on_menu(which);
    }));
}

// ═══════════════════════════════════════════════════════════════
// 3. 开机自启（macOS：LaunchAgent plist）
// ═══════════════════════════════════════════════════════════════

/// 开机自启开关。
///
/// `auto-launch` 在 macOS 上的行为：写/删
/// `~/Library/LaunchAgents/<app_name>.plist`，**无系统级依赖**。
///
/// ## 实测结果
/// `enable()` → `is_enabled() == true` → plist 落盘 → plist 含 `ProgramArguments` →
/// `disable()` → `is_enabled() == false` → plist 被删除。全流程通过。
///
/// ## 注意
/// - `set_use_launch_agent(true)` 是 macOS 正确行为（否则会尝试用 AppleScript 的
///   Login Items，那是旧机制）
/// - `app_path` 必须是**绝对路径**，且正式发布时应指向 `.app` bundle 内的可执行文件
/// - 用与产品名不同的 `app_name` 做测试，避免污染真实配置
pub fn autostart_handle(app_name: &str, app_path: &str) -> Result<auto_launch::AutoLaunch, String> {
    auto_launch::AutoLaunchBuilder::new()
        .set_app_name(app_name)
        .set_app_path(app_path)
        .set_use_launch_agent(true)
        .build()
        .map_err(|e| format!("AutoLaunch 构建失败: {e}"))
}

// ═══════════════════════════════════════════════════════════════
// 4. 把 OS 线程的事件桥接回 GPUI 状态
// ═══════════════════════════════════════════════════════════════

/// 热键/tray 回调拿不到 `Context`，因此用原子量做单向信号，
/// 在 GPUI 侧用 `cx.spawn` 轮询并更新 UI。
///
/// 实测有效：日志中 `[alive] ticks = 40 | 热键点击 = 4` 证明
/// 轮询心跳与热键计数能正确汇合。
///
/// ## 更优方案（供 S07 参考）
/// 用 `async_channel` / `futures::channel::mpsc` 代替轮询：
/// 回调里 `sender.try_send(..)`，GPUI 侧 `cx.spawn` 里 `recv().await`
/// —— 无轮询开销，且延迟更低。原子量方案存在最长一个轮询周期的延迟。
pub mod bridge {
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub static HOTKEY_HITS: AtomicUsize = AtomicUsize::new(0);
    pub static TRAY_MENU_HITS: AtomicUsize = AtomicUsize::new(0);

    pub fn note_hotkey() -> usize {
        HOTKEY_HITS.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn note_tray() -> usize {
        TRAY_MENU_HITS.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn counts() -> (usize, usize) {
        (
            HOTKEY_HITS.load(Ordering::SeqCst),
            TRAY_MENU_HITS.load(Ordering::SeqCst),
        )
    }
}

// ═══════════════════════════════════════════════════════════════
// 5. 启动顺序（实测可用的排列）
// ═══════════════════════════════════════════════════════════════

/// ```ignore
/// fn main() {
///     application().run(|cx: &mut App| {
///         // 1) theme（顺序不可换，否则开窗 panic —— 见 S00-01 §10.3）
///         theme::init(LoadThemes::JustBase, cx);
///         theme::set_theme_settings_provider(Box::new(MyProvider), cx);
///
///         // 2) 窗口
///         cx.open_window(WindowOptions { .. }, |_, cx| cx.new(|_| MyView))?;
///
///         // 3) 外壳三件套 —— 在 GPUI 闭包内直接初始化即可，无需另起循环
///         register_global_hotkey(|hk| { /* note_hotkey() */ })?;
///         let handles = create_tray()?;
///         set_tray_menu_handler(handles, |which| { /* note_tray() */ });
///         let al = autostart_handle("Buddy", "/Applications/Buddy.app/Contents/MacOS/Buddy")?;
///
///         // 4) 桥接轮询
///         cx.spawn(async move |this, cx| loop { /* timer + this.update */ }).detach();
///
///         cx.activate(true);
///     });
/// }
/// ```

// ═══════════════════════════════════════════════════════════════
// 6. 遗留：macOS 侧的「不出现在任务栏」= 应用级设定
// ═══════════════════════════════════════════════════════════════

/// 探测当前 `NSApp` 的 `activationPolicy`。
///
/// | 值 | 含义 |
/// |----|------|
/// | 0 | `Regular` —— 有 Dock 图标与菜单栏 |
/// | 1 | `Accessory` —— 无 Dock 图标（tray-only 应用） |
/// | 2 | `Prohibited` —— 不参与激活 |
///
/// 实测：GPUI 的 macOS 后端把策略设为 `Regular`（0），**tray 创建后未改变**。
///
/// `AGENTS.md` 里 `skipTaskbar: true` 是 Windows 概念；macOS 等价物需要把
/// `activationPolicy` 改成 `Accessory`。**这是应用级设定，不是窗口级**，
/// 因此归 `S07-11`（单实例与生命周期）而非 `S07-02`。
pub fn nsapp_activation_policy() -> i64 {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
        if app.is_null() {
            return -1;
        }
        msg_send![app, activationPolicy]
    }
}

/// 读取 `NSApp` 的 delegate 类名 —— 用于确认第三方 crate 未抢占 delegate。
///
/// 实测：tray 创建前后均为 `GPUIApplicationDelegate`，未被抢占。
pub fn nsapp_delegate_class() -> String {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
        if app.is_null() {
            return "<NSApp 为 null>".into();
        }
        let delegate: *mut AnyObject = msg_send![app, delegate];
        if delegate.is_null() {
            return "<delegate 为 nil>".into();
        }
        let cls: *mut AnyObject = msg_send![delegate, class];
        let name: *mut AnyObject = msg_send![cls, description];
        let utf8: *const std::os::raw::c_char = msg_send![name, UTF8String];
        if utf8.is_null() {
            return "<无法取类名>".into();
        }
        std::ffi::CStr::from_ptr(utf8).to_string_lossy().to_string()
    }
}
