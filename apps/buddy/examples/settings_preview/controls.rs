use super::*;

pub(crate) struct ControlsPreview {
    field: Entity<SettingsField>,
    select: Entity<SettingsSelect>,
    toggle_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    button_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    enabled: bool,
    button_hits: usize,
    toggle_focus: buddy_ui::gpui::FocusHandle,
    button_focus: buddy_ui::gpui::FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ControlsPreview {
    fn new(cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| SettingsField::with_id("preview-field", "输入名称", cx));
        let select = cx.new(|cx| SettingsSelect::new("preview-select", ["浅色", "深色"], 0, cx));
        let subscriptions = vec![
            cx.subscribe(&field, |_, _, _: &SettingsFieldEvent, cx| cx.notify()),
            cx.subscribe(&select, |_, _, _: &SettingsSelectChanged, cx| cx.notify()),
        ];
        Self {
            field,
            select,
            toggle_bounds: Rc::new(RefCell::new(None)),
            button_bounds: Rc::new(RefCell::new(None)),
            enabled: false,
            button_hits: 0,
            toggle_focus: cx.focus_handle(),
            button_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }
}

impl Render for ControlsPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let toggle = settings_controls::toggle("preview-toggle", self.enabled, cx)
            .track_focus(&self.toggle_focus)
            .on_click(cx.listener(|this, _, window, cx| {
                window.focus(&this.toggle_focus, cx);
                this.enabled = !this.enabled;
                cx.notify();
            }));
        let button = settings_controls::button("preview-button", "执行操作", cx)
            .track_focus(&self.button_focus)
            .on_click(cx.listener(|this, _, window, cx| {
                window.focus(&this.button_focus, cx);
                this.button_hits += 1;
                cx.notify();
            }));
        let toggle_bounds = self.toggle_bounds.clone();
        let toggle = div().relative().child(toggle).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, _, _| {
                    *toggle_bounds.borrow_mut() = Some(bounds);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        let button_bounds = self.button_bounds.clone();
        let button = div().relative().child(button).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, _, _| {
                    *button_bounds.borrow_mut() = Some(bounds);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        div()
            .id("settings-controls-preview")
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key != "tab" {
                    return;
                }
                let order = [
                    this.field.focus_handle(cx),
                    this.toggle_focus.clone(),
                    this.select.focus_handle(cx),
                    this.button_focus.clone(),
                ];
                let current = order.iter().position(|focus| focus.is_focused(window));
                let next = match current {
                    Some(i) if event.keystroke.modifiers.shift => {
                        (i + order.len() - 1) % order.len()
                    }
                    Some(i) => (i + 1) % order.len(),
                    None if event.keystroke.modifiers.shift => order.len() - 1,
                    None => 0,
                };
                window.focus(&order[next], cx);
                cx.stop_propagation();
            }))
            .size_full()
            .p(px(m::SPACE_6))
            .flex()
            .flex_col()
            .gap(px(m::SPACE_3))
            .bg(c.bg_canvas)
            .text_color(c.text_primary)
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_LG))
                    .child("S06 共用控件预览"),
            )
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_XS))
                    .text_color(c.text_muted)
                    .child(format!(
                        "开关：{}　按钮点击：{} 次",
                        if self.enabled { "开" } else { "关" },
                        self.button_hits
                    )),
            )
            .child(settings_controls::section(
                "文本输入",
                "单行设置字段",
                self.field.clone(),
                cx,
            ))
            .child(settings_controls::section(
                "启用状态",
                "点击切换受控开关",
                toggle,
                cx,
            ))
            .child(settings_controls::section(
                "外观",
                "点击或使用方向键选择",
                self.select.clone(),
                cx,
            ))
            .child(settings_controls::section(
                "操作",
                "按钮动作由页面接收",
                button,
                cx,
            ))
    }
}

pub(crate) fn open_controls(cx: &mut App) -> WindowHandle<ControlsPreview> {
    let bounds = Bounds::centered(None, size(px(560.0), px(640.0)), cx);
    cx.open_window(fixture::options(bounds), |_, cx| {
        cx.new(ControlsPreview::new)
    })
    .expect("打开控件预览窗口")
}

#[path = "controls_test.rs"]
pub(crate) mod testing;
