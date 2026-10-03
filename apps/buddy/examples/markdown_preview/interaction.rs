use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::time::Duration;

use buddy_ui::gpui::{
    AppContext, AsyncApp, Bounds, Entity, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, PlatformInput, WindowHandle, point, px,
};
use buddy_ui::markdown::zed_markdown::Markdown;

/// Counters used by the S07-07 event-boundary selftest.
pub(crate) struct InteractionState {
    pub(crate) blank_enabled: Cell<bool>,
    pub(crate) open_links: Cell<bool>,
    pub(crate) blank_mouse_down: Cell<usize>,
    pub(crate) url_click: Cell<usize>,
    pub(crate) checkbox_toggle: Cell<usize>,
    pub(crate) image_resolved: Cell<usize>,
    pub(crate) image_bounds: RefCell<BTreeMap<String, Bounds<Pixels>>>,
    pub(crate) code_bounds: Cell<Option<Bounds<Pixels>>>,
    pub(crate) task_bounds: Cell<Option<Bounds<Pixels>>>,
    pub(crate) hovered_url: Cell<bool>,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            blank_enabled: Cell::new(false),
            open_links: Cell::new(true),
            blank_mouse_down: Cell::new(0),
            url_click: Cell::new(0),
            checkbox_toggle: Cell::new(0),
            image_resolved: Cell::new(0),
            image_bounds: RefCell::new(BTreeMap::new()),
            code_bounds: Cell::new(None),
            task_bounds: Cell::new(None),
            hovered_url: Cell::new(false),
        }
    }
}

const INTERACTION_DOC: &str = r#"普通文字与[链接文字](https://example.com)

![无链接图片](fixture://unlinked.png)

[![有链接图片](fixture://linked.png)](https://example.com/image)

```rust
let copied = 42;
```

- [ ] 复选框

尾部空白区域"#;

fn draw(handle: WindowHandle<super::Preview>, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn dispatch(handle: WindowHandle<super::Preview>, input: PlatformInput, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(input, cx);
    });
    draw(handle, cx);
}

fn click(
    handle: WindowHandle<super::Preview>,
    position: buddy_ui::gpui::Point<buddy_ui::gpui::Pixels>,
    cx: &mut AsyncApp,
) {
    let modifiers = Modifiers::default();
    dispatch(
        handle,
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
    dispatch(
        handle,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    dispatch(
        handle,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
        }),
        cx,
    );
}

/// T52 is the native window proof; T08 only proves that Markdown's blank-area
/// callback does not steal text or child-control mouse events.
pub(crate) async fn selftest_t08(
    handle: WindowHandle<super::Preview>,
    md: &Entity<Markdown>,
    cx: &mut AsyncApp,
) -> bool {
    md.update(cx, |markdown, cx| markdown.replace(INTERACTION_DOC, cx));
    super::wait_parsed(md, INTERACTION_DOC.len(), cx).await;
    let state = handle
        .read_with(cx, |preview, _| preview.interaction.clone())
        .expect("预览窗口");
    state.blank_enabled.set(false);
    state.open_links.set(false);
    state.blank_mouse_down.set(0);
    state.url_click.set(0);
    state.checkbox_toggle.set(0);
    state.image_resolved.set(0);
    state.image_bounds.borrow_mut().clear();
    state.code_bounds.set(None);
    state.task_bounds.set(None);
    state.hovered_url.set(false);
    draw(handle, cx);
    state.blank_enabled.set(true);
    draw(handle, cx);

    let mut images_ready = false;
    for _ in 0..50 {
        let ready =
            {
                let bounds = state.image_bounds.borrow();
                ["fixture://unlinked.png", "fixture://linked.png"]
                    .iter()
                    .all(|url| {
                        bounds.get(*url).is_some_and(|bounds| {
                            bounds.size.width >= px(16.) && bounds.size.height >= px(16.)
                        })
                    })
                    && state.code_bounds.get().is_some_and(|bounds| {
                        bounds.size.width > px(0.) && bounds.size.height > px(0.)
                    })
                    && state.task_bounds.get().is_some_and(|bounds| {
                        bounds.size.width > px(0.) && bounds.size.height > px(0.)
                    })
            };
        if ready {
            images_ready = true;
            break;
        }
        draw(handle, cx);
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }

    // Text/control rows use the fixed 640×820 preview. Image rows are clicked
    // from the bounds reported by MarkdownElement::on_image_bounds below;
    // the code-copy row is accepted only when its clipboard side effect proves
    // that the child control was actually hit.
    let blank_before = state.blank_mouse_down.get();
    click(handle, point(px(610.), px(62.)), cx);
    let blank_area = state.blank_mouse_down.get() == blank_before + 1;

    let before = state.blank_mouse_down.get();
    click(handle, point(px(20.), px(62.)), cx);
    let text_kept = state.blank_mouse_down.get() == before;

    let before = state.blank_mouse_down.get();
    let link_point = (16..=300).find_map(|x| {
        state.hovered_url.set(false);
        let point = point(px(x as f32), px(62.));
        dispatch(
            handle,
            PlatformInput::MouseMove(MouseMoveEvent {
                position: point,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            cx,
        );
        state.hovered_url.get().then_some(point)
    });
    if let Some(link_point) = link_point {
        click(handle, link_point, cx);
    }
    let link_kept = link_point.is_some()
        && state.blank_mouse_down.get() == before
        && state.url_click.get() == 1;

    let unlinked_bounds = state
        .image_bounds
        .borrow()
        .get("fixture://unlinked.png")
        .copied();
    let before = state.blank_mouse_down.get();
    let unlinked_image_kept = unlinked_bounds.is_some_and(|bounds| {
        click(handle, bounds.center(), cx);
        state.blank_mouse_down.get() == before
    });

    let linked_bounds = state
        .image_bounds
        .borrow()
        .get("fixture://linked.png")
        .copied();
    let before = state.blank_mouse_down.get();
    let before_links = state.url_click.get();
    let linked_image_kept = linked_bounds.is_some_and(|bounds| {
        click(handle, bounds.center(), cx);
        state.blank_mouse_down.get() == before && state.url_click.get() == before_links + 1
    });

    let before = state.blank_mouse_down.get();
    let saved_clipboard = cx.update(|cx| cx.read_from_clipboard());
    let copy_point = state.code_bounds.get().map(|bounds| {
        point(
            bounds.origin.x + bounds.size.width - px(28.),
            bounds.origin.y + px(14.),
        )
    });
    if let Some(copy_point) = copy_point {
        click(handle, copy_point, cx);
    }
    let copied = cx
        .update(|cx| cx.read_from_clipboard())
        .and_then(|item| item.text())
        .unwrap_or_default();
    if let Some(saved_clipboard) = saved_clipboard {
        cx.update(|cx| cx.write_to_clipboard(saved_clipboard));
    }
    let copy_button_kept = copy_point.is_some()
        && state.blank_mouse_down.get() == before
        && copied.contains("let copied = 42;");

    let before = state.blank_mouse_down.get();
    let task_point = state.task_bounds.get().map(|bounds| bounds.center());
    if let Some(task_point) = task_point {
        click(handle, task_point, cx);
    }
    let checkbox_kept = task_point.is_some()
        && state.blank_mouse_down.get() == before
        && state.checkbox_toggle.get() == 0;

    // Give the final frame a chance to publish the observed canvas bounds.
    cx.background_executor()
        .timer(Duration::from_millis(20))
        .await;

    let images_loaded = state.image_resolved.get() >= 2
        && unlinked_bounds
            .is_some_and(|bounds| bounds.size.width > px(0.) && bounds.size.height > px(0.))
        && linked_bounds
            .is_some_and(|bounds| bounds.size.width > px(0.) && bounds.size.height > px(0.));
    let ok = images_ready
        && images_loaded
        && blank_area
        && text_kept
        && link_kept
        && linked_image_kept
        && unlinked_image_kept
        && copy_button_kept
        && checkbox_kept;
    println!(
        "T08: blank={} text={} link={} image(unlinked/linked)={}/{} loaded={} copy={} checkbox={} (url={}, toggle={})",
        blank_area,
        text_kept,
        link_kept,
        unlinked_image_kept,
        linked_image_kept,
        images_loaded,
        copy_button_kept,
        checkbox_kept,
        state.url_click.get(),
        state.checkbox_toggle.get()
    );
    println!(
        "T08 bounds: unlinked={unlinked_bounds:?} linked={linked_bounds:?} code={:?} task={:?} link_point={link_point:?} copy_point={copy_point:?}",
        state.code_bounds.get(),
        state.task_bounds.get(),
    );
    println!(
        "{} S07-07 Markdown 空白拖动不吞文字、链接、图片、代码复制按钮和复选框事件",
        if ok { "PASS" } else { "FAIL" }
    );
    state.blank_enabled.set(false);
    state.open_links.set(true);
    ok
}
