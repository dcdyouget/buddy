//! S05-16 空态页与无 Key 页的预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example pages_preview                 # 目检：两个 560×60 窗口（空态页、无 Key 页）
//! cargo run -p buddy-app --example pages_preview -- --error      # 空态页带错误提示（窗口加高到 160）
//! cargo run -p buddy-app --example pages_preview -- --selftest   # 自检后退出
//! ```
//!
//! 窗口无标题栏（`titlebar: None`）、背景透明：空态页只剩一枚浮动的输入气泡，与 v1 一致；
//! 窗口外壳 / 圆角 / 阴影 / 拖拽属 Phase 07。**Cmd+Q 或 Esc 退出。**
//!
//! 自检：
//! - T23 无 Key 页（S05-16）：真实点击面板、聚焦后 Enter / 空格都发出「前往设置」，其他键不发；
//! - T24 空态页（S05-16）：点「展开」发出展开；紧凑窗口（≤180px 高）中气泡撑满并垂直居中、较高窗口中贴底；
//!   多行草稿不撑高气泡；错误条出现在输入区上方，点关闭发出关闭事件。

use buddy_ui::chat::{
    composer::Composer,
    empty_page::{EmptyPage, EmptyPageEvent},
    no_key_page::{NoKeyPage, NoKeyPageEvent},
};
use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, Focusable, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PlatformInput, Render, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowOptions, point, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown;
use buddy_ui::theme_system::{Appearance, Theme, fonts};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const WIDTH: f32 = 560.0;
const COMPACT_HEIGHT: f32 = 60.0;
const TALL_HEIGHT: f32 = 260.0;
/// 自检中的错误文案
const ERROR_TEXT: &str = "网络错误，请重试";

type Log = Rc<RefCell<Vec<&'static str>>>;

fn open_empty(cx: &mut App, origin_y: f32, height: f32, log: Log) -> WindowHandle<EmptyPage> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(height)), cx);
    let bounds = Bounds::new(point(bounds.origin.x, px(origin_y)), bounds.size);
    let handle = cx
        .open_window(options(bounds), |window, cx| {
            let composer = cx.new(|cx| Composer::new(window, cx));
            composer.update(cx, |c, cx| c.set_standalone(true, cx));
            let page = cx.new(|_| EmptyPage::new(composer));
            cx.subscribe(&page, move |page, event: &EmptyPageEvent, cx| {
                match event {
                    EmptyPageEvent::Expand => log.borrow_mut().push("expand"),
                    EmptyPageEvent::DismissError => {
                        log.borrow_mut().push("dismiss");
                        page.update(cx, |p, cx| p.set_error(None, cx));
                    }
                }
                println!("空态页事件：{event:?}");
            })
            .detach();
            page
        })
        .expect("open_window 失败");
    handle
}

fn open_no_key(cx: &mut App, origin_y: f32, log: Log) -> WindowHandle<NoKeyPage> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(COMPACT_HEIGHT)), cx);
    let bounds = Bounds::new(point(bounds.origin.x, px(origin_y)), bounds.size);
    cx.open_window(options(bounds), |_, cx| {
        let page = cx.new(NoKeyPage::new);
        cx.subscribe(&page, move |_, event: &NoKeyPageEvent, _| {
            log.borrow_mut().push("settings");
            println!("无 Key 页事件：{event:?}");
        })
        .detach();
        page
    })
    .expect("open_window 失败")
}

fn options(bounds: Bounds<buddy_ui::gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
}

async fn draw<V: Render + 'static>(handle: WindowHandle<V>, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

async fn press<V: Render + 'static>(handle: WindowHandle<V>, keys: &str, cx: &mut AsyncApp) {
    let key = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse(keys).expect("按键"), is_held: false, prefer_character_input: false });
    let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(key, cx));
    draw(handle, cx).await;
}

/// 真实点击（移动 → 按下 → 抬起）
async fn click<V: Render + 'static>(handle: WindowHandle<V>, x: f32, y: f32, cx: &mut AsyncApp) {
    let p = point(px(x), px(y));
    for e in [
        PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: Modifiers::default() }),
        PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: p, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }),
        PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: p, modifiers: Modifiers::default(), click_count: 1 }),
    ] {
        let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
        draw(handle, cx).await;
    }
}

/// T23 无 Key 页
async fn selftest_no_key(handle: WindowHandle<NoKeyPage>, log: &Log, cx: &mut AsyncApp) -> bool {
    draw(handle, cx).await;
    let count = |log: &Log| log.borrow().len();
    click(handle, WIDTH / 2.0, COMPACT_HEIGHT / 2.0, cx).await;
    let clicked = count(log) == 1;
    let focus = handle.read_with(cx, |p, cx| p.focus_handle(cx)).unwrap();
    let _ = cx.update_window(handle.into(), |_, window, cx| window.focus(&focus, cx));
    draw(handle, cx).await;
    press(handle, "enter", cx).await;
    let enter = count(log) == 2;
    press(handle, "space", cx).await;
    let space = count(log) == 3;
    press(handle, "a", cx).await;
    press(handle, "tab", cx).await;
    let others_ignored = count(log) == 3;
    // 面板之外（窗口四角之外不存在；点在面板内左端图标处仍算面板）
    click(handle, 20.0, COMPACT_HEIGHT / 2.0, cx).await;
    let icon_click = count(log) == 4;
    println!("T23: 点击面板 {clicked}；聚焦后 Enter {enter}、空格 {space}、其他键不触发 {others_ignored}；点击图标处 {icon_click}");
    let ok = clicked && enter && space && others_ignored && icon_click;
    println!("{} S05-16 T23 无 Key 页（点击 / Enter / 空格前往设置）", if ok { "PASS" } else { "FAIL" });
    ok
}

/// T24 空态页
async fn selftest_empty(compact: WindowHandle<EmptyPage>, compact_log: &Log, tall: WindowHandle<EmptyPage>, tall_log: &Log, cx: &mut AsyncApp) -> bool {
    for _ in 0..3 {
        draw(compact, cx).await;
        draw(tall, cx).await;
    }
    // 展开按钮：顶部居中，24×20，距顶 2
    click(compact, WIDTH / 2.0, 2.0 + 10.0, cx).await;
    let expand = compact_log.borrow().as_slice() == ["expand"];
    // 展开按钮之外不触发
    click(compact, 40.0, 30.0, cx).await;
    let expand_only_on_button = compact_log.borrow().len() == 1;

    let text_bounds = |handle: WindowHandle<EmptyPage>, cx: &mut AsyncApp| {
        handle
            .read_with(cx, |p, cx| p.composer().read(cx).text_area().read(cx).painted_bounds_for_test())
            .unwrap()
            .expect("输入框尚未绘制")
    };
    // 紧凑窗口：气泡撑满高度，输入框在窗口垂直中央
    let c = text_bounds(compact, cx);
    let center = f32::from(c.origin.y + c.size.height / 2.0);
    // 较高窗口：气泡贴底
    let t = text_bounds(tall, cx);
    let tall_center = f32::from(t.origin.y + t.size.height / 2.0);
    let filled = (center - COMPACT_HEIGHT / 2.0).abs() <= 1.0;
    let bottom_aligned = tall_center > TALL_HEIGHT - 40.0 && tall_center < TALL_HEIGHT - 10.0;

    // 多行草稿不撑高（v1 `disableAutoResize`）：输入框保持一行高
    for h in [compact, tall] {
        let composer = h.read_with(cx, |p, _| p.composer().clone()).unwrap();
        let _ = cx.update_window(h.into(), |_, _, cx| composer.update(cx, |c, cx| c.set_draft("第一行\n第二行\n第三行\n第四行", cx)));
        draw(h, cx).await;
        draw(h, cx).await;
    }
    let (c2, t2) = (text_bounds(compact, cx), text_bounds(tall, cx));
    let single_line = 24.0;
    let no_growth = (f32::from(c2.size.height) - single_line).abs() < 0.5 && (f32::from(t2.size.height) - single_line).abs() < 0.5;

    // 错误条：出现在输入区上方；点关闭发出关闭事件（自检的订阅方随即清除错误）
    let _ = tall.update(cx, |p, _, cx| p.set_error(Some(ERROR_TEXT.into()), cx));
    draw(tall, cx).await;
    draw(tall, cx).await;
    let shown = tall.read_with(cx, |p, _| p.error() == Some(ERROR_TEXT)).unwrap();
    // 关闭按钮（24×24）在条右端：窗口宽 − 条外边距 12 − 条内边距 12 − 半个按钮 12；纵向位置由布局决定，自上而下扫描
    let before = tall_log.borrow().len();
    let mut dismissed_at = None;
    let mut y = t2.origin.y.into();
    let top: f32 = 0.0;
    y = f32::min(y, TALL_HEIGHT);
    for step in 0..(y as usize / 2) {
        let yy = top + step as f32 * 2.0;
        click(tall, WIDTH - 12.0 - 12.0 - 12.0, yy, cx).await;
        if tall_log.borrow().len() > before {
            dismissed_at = Some(yy);
            break;
        }
    }
    draw(tall, cx).await;
    let cleared = tall.read_with(cx, |p, _| p.error().is_none()).unwrap();
    let banner_above_input = dismissed_at.is_some_and(|yy| yy < f32::from(t2.origin.y));
    println!(
        "T24: 展开 {expand}（按钮之外不触发 {expand_only_on_button}）；紧凑窗口输入框中心 {center:.1}（窗口中心 {:.1}）、较高窗口 {tall_center:.1}/{TALL_HEIGHT}；\
         四行草稿时输入框高 {:.1} / {:.1}（一行 {single_line}）；错误条显示 {shown}，关闭点 y={dismissed_at:?}（输入框顶 {:.1}），已清除 {cleared}",
        COMPACT_HEIGHT / 2.0,
        f32::from(c2.size.height),
        f32::from(t2.size.height),
        f32::from(t2.origin.y),
    );
    let ok = expand && expand_only_on_button && filled && bottom_aligned && no_growth && shown && banner_above_input && cleared;
    println!("{} S05-16 T24 空态页（展开 / 独立气泡撑满与贴底 / 不撑高 / 错误条）", if ok { "PASS" } else { "FAIL" });
    ok
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    let self_test = args.iter().any(|a| a == "--selftest");
    let with_error = args.iter().any(|a| a == "--error");
    application().with_assets(buddy_ui::icons::Assets).run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
        buddy_ui::chat::init(cx);
        // 无标题栏，没有关闭按钮：Cmd+Q / Esc 退出（app 不直接依赖 gpui，故不用 `actions!`）
        cx.observe_keystrokes(|event, _, cx| {
            if event.keystroke.unparse() == "cmd-q" || event.keystroke.key == "escape" {
                cx.quit();
            }
        })
        .detach();

        let log: Log = Rc::default();
        if self_test {
            let no_key_log: Log = Rc::default();
            let compact = open_empty(cx, 100.0, COMPACT_HEIGHT, log.clone());
            let no_key = open_no_key(cx, 220.0, no_key_log.clone());
            let tall_log: Log = Rc::default();
            let tall = open_empty(cx, 340.0, TALL_HEIGHT, tall_log.clone());
            cx.spawn(async move |cx: &mut AsyncApp| {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let t23 = selftest_no_key(no_key, &no_key_log, cx).await;
                let t24 = selftest_empty(compact, &log, tall, &tall_log, cx).await;
                std::process::exit(if t23 && t24 { 0 } else { 1 });
            })
            .detach();
        } else {
            let height = if with_error { 160.0 } else { COMPACT_HEIGHT };
            let empty = open_empty(cx, 200.0, height, log.clone());
            open_no_key(cx, 200.0 + height + 40.0, log);
            if with_error {
                let _ = empty.update(cx, |p, _, cx| p.set_error(Some("网络连接失败，请检查网络后重试".into()), cx));
            }
        }
    });
}
