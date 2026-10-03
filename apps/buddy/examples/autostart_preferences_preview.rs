//! S07-09/S07-10 配置队列独立自测。
//!
//! 该示例只使用真实 `PageRouter` 与 `ChatEngine`，但把开机自启后端替换为
//! 可注入的沙盒实现。它不创建托盘、不修改正式配置，也不需要修改 examples
//! manifest：
//!
//! ```text
//! cargo run -p buddy-app --example autostart_preferences_preview -- --selftest
//! ```

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Theme};
use buddy_engine::storage;
use buddy_ui::chat::router::{Loaded, PageRouter};
use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::settings::SettingsEvent;
use buddy_ui::shell::autostart::AutostartBackend;
use buddy_ui::theme_system::{Appearance, Theme as UiTheme, fonts};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[path = "autostart_preferences_preview/native.rs"]
mod native;

const WIDTH: f32 = 520.0;
const HEIGHT: f32 = 360.0;

#[derive(Clone)]
struct FakeAutostart {
    enabled: Rc<Cell<bool>>,
    fail_set: Rc<Cell<bool>>,
    calls: Rc<RefCell<Vec<bool>>>,
}

impl FakeAutostart {
    fn new(enabled: bool) -> Self {
        Self {
            enabled: Rc::new(Cell::new(enabled)),
            fail_set: Rc::new(Cell::new(false)),
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn set_failure(&self, failure: bool) {
        self.fail_set.set(failure);
    }
}

impl AutostartBackend for FakeAutostart {
    fn query(&self) -> Result<bool, String> {
        Ok(self.enabled.get())
    }

    fn set(&self, enabled: bool) -> Result<(), String> {
        self.calls.borrow_mut().push(enabled);
        if self.fail_set.get() {
            return Err("测试后端故意拒绝自启切换".into());
        }
        self.enabled.set(enabled);
        Ok(())
    }
}

/// 将 `config.json` 临时替换为目录，迫使真实 engine 原子替换失败。
/// Drop 时恢复原始字节，避免把故障注入留在沙盒中。
struct ConfigPathGuard {
    path: PathBuf,
    original: Vec<u8>,
    restored: bool,
}

impl ConfigPathGuard {
    fn block(path: PathBuf) -> Result<Self, String> {
        let original = fs::read(&path).map_err(|error| format!("读取配置样本失败：{error}"))?;
        fs::remove_file(&path).map_err(|error| format!("移除配置样本失败：{error}"))?;
        fs::create_dir(&path).map_err(|error| format!("创建配置阻断目录失败：{error}"))?;
        Ok(Self {
            path,
            original,
            restored: false,
        })
    }

    fn restore(&mut self) -> Result<(), String> {
        let _ = fs::remove_file(&self.path);
        fs::remove_dir(&self.path).map_err(|error| format!("移除配置阻断目录失败：{error}"))?;
        fs::write(&self.path, &self.original)
            .map_err(|error| format!("恢复配置样本失败：{error}"))?;
        let restored =
            fs::read(&self.path).map_err(|error| format!("读回配置样本失败：{error}"))?;
        if restored != self.original {
            return Err("恢复后的配置样本字节不一致".into());
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for ConfigPathGuard {
    fn drop(&mut self) {
        if !self.restored {
            if let Err(error) = self.restore() {
                eprintln!("FAIL S07-10 preferences cleanup：{error}");
            }
        }
    }
}

fn sandbox(root: &Path, name: &str) -> Result<PathBuf, String> {
    let path = root.join(name);
    fs::create_dir_all(&path).map_err(|error| format!("创建 {name} 沙盒失败：{error}"))?;
    Ok(path)
}

fn config() -> AppConfig {
    let mut config = AppConfig::default();
    config.theme = Theme::Dark;
    config.auto_start = false;
    config.hotkey = "CmdOrCtrl+Shift+P".into();
    config.allowed_paths = vec!["/tmp/autostart-preferences-sentinel".into()];
    config
}

fn window_options(bounds: Bounds<buddy_ui::gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
}

fn open_router(
    cx: &mut App,
    data_dir: PathBuf,
    loaded_config: AppConfig,
) -> WindowHandle<PageRouter> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let engine = ChatEngine::new(data_dir);
    let loaded = Loaded {
        config: loaded_config,
        history: Vec::new(),
        offset: 0,
    };
    cx.open_window(window_options(bounds), |window, cx| {
        cx.new(|cx| PageRouter::new(engine, loaded, window, cx))
    })
    .expect("打开自启配置自测窗口")
}

async fn toggle(
    handle: WindowHandle<PageRouter>,
    backend: Rc<dyn AutostartBackend>,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let task = handle
        .update(cx, |router, _, cx| router.toggle_autostart(backend, cx))
        .map_err(|error| format!("Router 已销毁：{error}"))?;
    task.await
}

fn router_config(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> Result<AppConfig, String> {
    handle
        .read_with(cx, |router, _| router.config().clone())
        .map_err(|error| format!("读取 Router 配置失败：{error}"))
}

async fn test_success_and_queue(root: &Path, cx: &mut AsyncApp) -> Result<(), String> {
    let dir = sandbox(root, "success-and-queue")?;
    let initial = config();
    storage::save_config(&dir, &initial)?;
    let handle = cx.update(|app| open_router(app, dir.clone(), initial.clone()));
    let fake = FakeAutostart::new(false);
    let backend: Rc<dyn AutostartBackend> = Rc::new(fake.clone());

    // 不等待第一项；主题保存插在两次 toggle 之间，三项必须复用同一队列。
    let first = handle
        .update(cx, |router, _, cx| {
            router.toggle_autostart(backend.clone(), cx)
        })
        .map_err(|error| format!("启动第一项自启事务失败：{error}"))?;
    let settings = handle
        .read_with(cx, |router, _| router.settings_view().clone())
        .map_err(|error| format!("读取设置实体失败：{error}"))?;
    handle
        .update(cx, |router, _, cx| router.open_settings(cx))
        .map_err(|error| format!("打开设置页失败：{error}"))?;
    settings.update(cx, |_, cx| {
        cx.emit(SettingsEvent::ThemeChanged(Theme::Light))
    });
    let second = handle
        .update(cx, |router, _, cx| router.toggle_autostart(backend, cx))
        .map_err(|error| format!("启动第二项自启事务失败：{error}"))?;
    let first_result = first.await?;
    let second_result = second.await?;
    let disk = storage::get_config(&dir)?;
    let visible = router_config(handle, cx)?;

    if first_result != true
        || second_result != false
        || fake.enabled.get()
        || disk.auto_start
        || visible.auto_start
        || !matches!(&disk.theme, Theme::Light)
        || !matches!(&visible.theme, Theme::Light)
        || disk.hotkey != "CmdOrCtrl+Shift+P"
        || visible.hotkey != "CmdOrCtrl+Shift+P"
        || disk.allowed_paths != vec!["/tmp/autostart-preferences-sentinel".to_string()]
        || visible.allowed_paths != vec!["/tmp/autostart-preferences-sentinel".to_string()]
    {
        return Err(format!(
            "成功/队列断言失败：first={first_result} second={second_result} os={} disk={} visible={} theme={:?}/{:?} hotkey={:?}/{:?} paths={:?}/{:?}",
            fake.enabled.get(),
            disk.auto_start,
            visible.auto_start,
            disk.theme,
            visible.theme,
            disk.hotkey,
            visible.hotkey,
            disk.allowed_paths,
            visible.allowed_paths
        ));
    }
    if fake.calls.borrow().as_slice() != [true, false] {
        return Err(format!("队列 OS 调用顺序错误：{:?}", fake.calls.borrow()));
    }
    println!(
        "PASS S07-10 preferences success-queue first={first_result} second={second_result} os={} disk={} visible={} theme=light fields_preserved=true",
        fake.enabled.get(),
        disk.auto_start,
        visible.auto_start
    );
    Ok(())
}

async fn test_save_failure_rolls_back(root: &Path, cx: &mut AsyncApp) -> Result<(), String> {
    let dir = sandbox(root, "save-failure")?;
    let initial = config();
    storage::save_config(&dir, &initial)?;
    let before_bytes = fs::read(dir.join("config.json")).map_err(|error| error.to_string())?;
    let handle = cx.update(|app| open_router(app, dir.clone(), initial));
    let fake = FakeAutostart::new(false);
    let backend: Rc<dyn AutostartBackend> = Rc::new(fake.clone());
    let mut guard = ConfigPathGuard::block(dir.join("config.json"))?;
    let result = toggle(handle, backend, cx).await;
    let restore = guard.restore();
    drop(guard);
    restore?;
    let after_bytes = fs::read(dir.join("config.json")).map_err(|error| error.to_string())?;
    let disk = storage::get_config(&dir)?;
    let visible = router_config(handle, cx)?;
    if result.is_ok()
        || fake.enabled.get()
        || disk.auto_start
        || visible.auto_start
        || before_bytes != after_bytes
        || fake.calls.borrow().as_slice() != [true, false]
    {
        return Err(format!(
            "写盘失败回滚断言失败：result={result:?} os={} disk={} visible={} bytes_equal={} calls={:?}",
            fake.enabled.get(),
            disk.auto_start,
            visible.auto_start,
            before_bytes == after_bytes,
            fake.calls.borrow()
        ));
    }
    println!(
        "PASS S07-10 preferences save-rollback result_err={} os={} disk={} visible={} bytes_equal=true",
        result.is_err(),
        fake.enabled.get(),
        disk.auto_start,
        visible.auto_start
    );
    Ok(())
}

async fn test_os_failure_does_not_save(root: &Path, cx: &mut AsyncApp) -> Result<(), String> {
    let dir = sandbox(root, "os-failure")?;
    let initial = config();
    storage::save_config(&dir, &initial)?;
    let before_bytes = fs::read(dir.join("config.json")).map_err(|error| error.to_string())?;
    let handle = cx.update(|app| open_router(app, dir.clone(), initial));
    let fake = FakeAutostart::new(false);
    fake.set_failure(true);
    let backend: Rc<dyn AutostartBackend> = Rc::new(fake.clone());
    let result = toggle(handle, backend, cx).await;
    let after_bytes = fs::read(dir.join("config.json")).map_err(|error| error.to_string())?;
    let disk = storage::get_config(&dir)?;
    let visible = router_config(handle, cx)?;
    if result.is_ok()
        || fake.enabled.get()
        || disk.auto_start
        || visible.auto_start
        || before_bytes != after_bytes
        || fake.calls.borrow().as_slice() != [true]
    {
        return Err(format!(
            "OS 失败断言失败：result={result:?} os={} disk={} visible={} bytes_equal={} calls={:?}",
            fake.enabled.get(),
            disk.auto_start,
            visible.auto_start,
            before_bytes == after_bytes,
            fake.calls.borrow()
        ));
    }
    println!(
        "PASS S07-10 preferences os-failure result_err={} os={} disk={} visible={} bytes_equal=true",
        result.is_err(),
        fake.enabled.get(),
        disk.auto_start,
        visible.auto_start
    );
    Ok(())
}

async fn run(cx: &mut AsyncApp) -> bool {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/buddy-autostart-preferences-preview")
        .join(std::process::id().to_string());
    if let Err(error) = fs::create_dir_all(&root) {
        eprintln!("[S07-09/S07-10] FAIL：创建总沙盒失败：{error}");
        return false;
    }

    let mut ok = true;
    for (label, result) in [
        ("success-queue", test_success_and_queue(&root, cx).await),
        (
            "save-rollback",
            test_save_failure_rolls_back(&root, cx).await,
        ),
        ("os-failure", test_os_failure_does_not_save(&root, cx).await),
        ("native-save-rollback", native::run(cx, &root).await),
    ] {
        match result {
            Err(error) => {
                ok = false;
                eprintln!("FAIL S07-10 preferences {label}：{error}");
            }
            Ok(()) => {}
        }
    }
    match fs::remove_dir_all(&root) {
        Ok(()) => ok,
        Err(error) => {
            eprintln!("FAIL S07-10 preferences cleanup：{error}");
            false
        }
    }
}

fn main() {
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            buddy_ui::init_theme(cx);
            UiTheme::install(Appearance::Light, cx);
            fonts::install_text_rendering(cx);
            buddy_ui::markdown::init(cx);
            buddy_ui::chat::init(cx);
            buddy_ui::chat_bridge::init(cx);
            cx.spawn(async move |cx: &mut AsyncApp| {
                let ok = run(cx).await;
                std::process::exit(if ok { 0 } else { 1 });
            })
            .detach();
        });
}
