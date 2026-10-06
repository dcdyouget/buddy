//! 模型选择菜单—— 对应 v1 `ModelDropdown.tsx`、`.model-dropdown*` 样式与 `src/utils/modelMenu.ts`
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 优先用系统原生菜单（独立于窗口绘制，紧凑气泡无需扩高），失败才用页内下拉 | GPUI 没有原生弹出菜单（macOS 拒绝 `WindowKind::AnchoredPopup`），改用一个独立的无边框 `PopUp` 窗口承载**下拉的外观**，按模型按钮的屏幕位置摆放 —— 同样不受紧凑窗口大小限制 |
//! | 只列出「Provider 启用了的」模型；空列表显示「暂无已启用模型，请前往设置添加」 | [`menu_rows`] |
//! | 行：名称 + `Provider · N K 上下文 · N ms`；当前模型高亮底 + 对勾；悬停凹陷底 | [`ModelMenu`] |
//! | 面板：宽 272、最高 320、圆角 lg、`--composer-surface`、`--shadow-floating-md`；Esc / 点击外部关闭；选中后 120 ms 再关闭 | 同（点击外部 = 窗口失去激活）。**阴影用系统窗口阴影**：窗口就是面板大小，不自绘阴影也不留透明边距 —— 曾自绘 12px 边距 + 阴影，macOS 的系统阴影沿着那圈半透明渐变再画一层，菜单外多出一圈矩形外边缘（目检 #18 反馈） |
//! | 边框 `--glass-outline` | 改用 `--border-default`（同 Composer） |
//!
//! 选择结果经回调交给路由器，由它保存配置（v1 `setDefaultModel`）。

use crate::icons::{IconName, icon};
#[cfg(target_os = "windows")]
use crate::shell::{AppShell, runtime, visibility};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::models::{ModelInfo, ProviderConfig};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, FocusHandle, Focusable, FontWeight, KeyDownEvent,
    Pixels, Render, SharedString, Task, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, div, point, prelude::*, px, size,
};
use std::rc::Rc;
use std::time::Duration;

/// 面板宽度（v1 `--space-12 × 5 + --space-8`）
pub const MENU_WIDTH: f32 = 272.0;
/// 面板最大高度（v1 `max-height: 320px`）
pub const MENU_MAX_HEIGHT: f32 = 320.0;
/// 行最小高度（v1 `--space-12`）
pub const ROW_HEIGHT: f32 = 48.0;
/// 空列表时的高度（v1 内边距 space-5 × 2 + 一行 13px/20px 文字）
const EMPTY_HEIGHT: f32 = 60.0;
/// 面板与模型按钮的间距（v1 `margin-bottom: space-2`）
const GAP: f32 = 8.0;
/// 选中后到关闭的延迟（v1 `setTimeout(onClose, 120)`）
const CLOSE_DELAY: Duration = Duration::from_millis(120);

/// 菜单中的一行
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRow {
    /// 模型 id
    pub id: String,
    /// 显示名
    pub name: String,
    /// 副标题：`Provider · 128K 上下文 · 230ms`
    pub detail: String,
}

/// v1 副标题：`{provider} · ` + `{N}K 上下文` + ` · ` + `{ms}ms`（缺项省略）
pub fn detail_text(provider: Option<&str>, context_window: u32, latency_ms: Option<u32>) -> String {
    let mut text = provider.map(|p| format!("{p} · ")).unwrap_or_default();
    if context_window > 0 {
        text.push_str(&format!(
            "{}K 上下文",
            (context_window as f64 / 1000.0).round() as u32
        ));
    }
    if context_window > 0 && latency_ms.is_some() {
        text.push_str(" · ");
    }
    if let Some(ms) = latency_ms {
        text.push_str(&format!("{ms}ms"));
    }
    text
}

/// 可选模型：所属 Provider 启用了该模型（v1 `enabledModels`）
pub fn menu_rows(models: &[ModelInfo], providers: &[ProviderConfig]) -> Vec<ModelRow> {
    models
        .iter()
        .filter(|model| {
            providers
                .iter()
                .any(|p| p.id == model.provider_id && p.enabled_model_ids.contains(&model.id))
        })
        .map(|model| {
            let provider = providers
                .iter()
                .find(|p| p.id == model.provider_id)
                .map(|p| p.name.as_str());
            ModelRow {
                id: model.id.clone(),
                name: model.display_name.clone(),
                detail: detail_text(provider, model.context_window, model.latency_ms),
            }
        })
        .collect()
}

/// 面板尺寸（不含阴影边距）
pub fn menu_size(rows: usize) -> (f32, f32) {
    let inner = if rows == 0 {
        EMPTY_HEIGHT
    } else {
        (rows as f32 * ROW_HEIGHT).min(MENU_MAX_HEIGHT)
    };
    (MENU_WIDTH, inner + 2.0)
}

/// 选择回调
pub type OnSelect = Rc<dyn Fn(String, &mut App)>;

/// 模型菜单
pub struct ModelMenu {
    rows: Vec<ModelRow>,
    selected: String,
    /// 刚点选、尚在关闭延迟内的模型（v1 `pendingSelectedId`：先给出选中反馈）
    pending: Option<String>,
    focus: FocusHandle,
    on_select: OnSelect,
    parent: AnyWindowHandle,
    /// 已经历过一次「激活」；之后失去激活才算点击了外部
    seen_active: bool,
    closing: Option<Task<()>>,
    #[cfg(target_os = "windows")]
    deactivation_check: Option<Task<()>>,
    _subscription: gpui::Subscription,
}

impl Focusable for ModelMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ModelMenu {
    fn new(
        rows: Vec<ModelRow>,
        selected: String,
        on_select: OnSelect,
        parent: AnyWindowHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // 点击外部：窗口失去激活（首次「激活」之前的状态变化忽略，避免刚打开就被关掉）
        let subscription = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.seen_active = true;
                #[cfg(target_os = "windows")]
                {
                    this.deactivation_check = None;
                }
            } else if this.seen_active && this.closing.is_none() {
                #[cfg(target_os = "windows")]
                {
                    let parent = this.parent;
                    this.deactivation_check = Some(cx.spawn_in(window, async move |this, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(20))
                            .await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            let buddy_window_is_foreground =
                                visibility::foreground_window_belongs_to_current_process();
                            this.close_without_parent_focus(window);
                            if !buddy_window_is_foreground {
                                if let Some(parent) = parent.downcast::<AppShell>() {
                                    runtime::request_hide(parent, cx);
                                }
                            }
                        });
                    }));
                }
                #[cfg(not(target_os = "windows"))]
                this.close(window, cx);
            }
        });
        Self {
            rows,
            selected,
            pending: None,
            focus,
            on_select,
            parent,
            seen_active: window.is_window_active(),
            closing: None,
            #[cfg(target_os = "windows")]
            deactivation_check: None,
            _subscription: subscription,
        }
    }

    /// 当前行（自检用）
    pub fn rows(&self) -> &[ModelRow] {
        &self.rows
    }

    /// 当前高亮的模型 id（自检用）
    pub fn highlighted(&self) -> &str {
        self.pending.as_deref().unwrap_or(&self.selected)
    }

    /// 点选一行：先给出选中反馈并通知选择，120 ms 后关闭
    pub fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing.is_some() {
            return;
        }
        self.pending = Some(id.clone());
        (self.on_select)(id, cx);
        cx.notify();
        self.closing = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CLOSE_DELAY).await;
            let _ = this.update_in(cx, |this, window, cx| this.close(window, cx));
        }));
    }

    /// 关闭菜单窗口，并把焦点还给父窗口
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = self.parent;
        self.close_without_parent_focus(window);
        cx.defer(move |cx| {
            let _ = parent.update(cx, |_, window, _| window.activate_window());
        });
    }

    fn close_without_parent_focus(&mut self, window: &mut Window) {
        window.remove_window();
    }
}

impl Render for ModelMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let c = theme.colors;
        let (_, height) = menu_size(self.rows.len());
        let highlighted = self.highlighted().to_string();
        let panel = div()
            .id("model-menu")
            .w(px(MENU_WIDTH))
            .h(px(height))
            .overflow_y_scroll()
            .rounded(px(m::RADIUS_LG))
            .border_1()
            .border_color(c.border_default)
            .bg(c.composer_surface)
            .when(self.rows.is_empty(), |d| {
                d.flex()
                    .items_center()
                    .justify_center()
                    .px(px(m::SPACE_4))
                    .text_size(px(m::FONT_SIZE_BASE))
                    .text_color(c.text_muted)
                    .child("暂无已启用模型，请前往设置添加")
            })
            .children(self.rows.iter().map(|row| {
                let selected = row.id == highlighted;
                let id = row.id.clone();
                div()
                    .id(SharedString::from(format!("model-row-{}", row.id)))
                    .flex()
                    .items_center()
                    .gap(px(m::SPACE_2))
                    .w_full()
                    .min_h(px(ROW_HEIGHT))
                    .px(px(m::SPACE_3))
                    .py(px(m::SPACE_2))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(c.primary_tint_soft))
                    .when(!selected, |d| d.hover(|s| s.bg(c.bg_sunken)))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select(id.clone(), window, cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(m::FONT_SIZE_BASE))
                                    .font_weight(FontWeight(500.0))
                                    .text_color(c.text_primary)
                                    .child(SharedString::from(row.name.clone())),
                            )
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(m::FONT_SIZE_SM))
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(row.detail.clone())),
                            ),
                    )
                    .when(selected, |d| {
                        d.child(
                            div()
                                .text_color(c.buddy_primary)
                                .child(icon(IconName::Check, px(14.0))),
                        )
                    })
            }));
        div()
            .id("model-menu-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    this.close(window, cx);
                }
            }))
            .size_full()
            .child(panel)
    }
}

/// 打开菜单。`anchor` 是模型按钮在父窗口中的边界（窗口坐标）。
///
/// 位置：面板右缘对齐父窗口右缘内 8px（v1 `right: space-2`），底边在按钮上方 8px；
/// 上方放不下（会顶出屏幕上缘）时改放到按钮下方。
pub fn open_model_menu(
    parent: &Window,
    parent_handle: AnyWindowHandle,
    anchor: Bounds<Pixels>,
    rows: Vec<ModelRow>,
    selected: String,
    on_select: OnSelect,
    cx: &mut App,
) -> Option<WindowHandle<ModelMenu>> {
    let (width, height) = menu_size(rows.len());
    let origin = parent.bounds().origin;
    let right = origin.x + parent.bounds().size.width - px(m::SPACE_2);
    let mut top = origin.y + anchor.top() - px(GAP) - px(height);
    if f32::from(top) < 24.0 {
        top = origin.y + anchor.bottom() + px(GAP);
    }
    let bounds = Bounds::new(point(right - px(width), top), size(px(width), px(height)));
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| ModelMenu::new(rows, selected, on_select, parent_handle, window, cx))
        },
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, provider: &str, name: &str, ctx: u32, latency: Option<u32>) -> ModelInfo {
        serde_json::from_value(serde_json::json!({
            "id": id, "provider_id": provider, "display_name": name, "context_window": ctx, "latency_ms": latency
        }))
        .unwrap()
    }

    fn provider(id: &str, name: &str, enabled: &[&str]) -> ProviderConfig {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": name, "base_url": "http://x", "api_key": "k", "enabled_model_ids": enabled
        }))
        .unwrap()
    }

    #[test]
    fn detail_follows_v1_template() {
        assert_eq!(
            detail_text(Some("OpenAI"), 128000, Some(230)),
            "OpenAI · 128K 上下文 · 230ms"
        );
        assert_eq!(
            detail_text(Some("OpenAI"), 128000, None),
            "OpenAI · 128K 上下文"
        );
        assert_eq!(detail_text(None, 200000, Some(5)), "200K 上下文 · 5ms");
        assert_eq!(
            detail_text(Some("X"), 0, Some(9)),
            "X · 9ms",
            "上下文为 0 时不显示，也没有多余分隔"
        );
        assert_eq!(detail_text(None, 0, None), "");
    }

    #[test]
    fn rows_list_only_models_the_provider_enabled() {
        let providers = vec![
            provider("p1", "甲", &["p1::a", "p1::b"]),
            provider("p2", "乙", &["p2::c"]),
        ];
        let models = vec![
            model("p1::a", "p1", "A", 8000, None),
            model("p1::x", "p1", "X（未启用）", 8000, None),
            model("p2::c", "p2", "C", 16000, Some(10)),
            model("p3::z", "p3", "Z（Provider 已删）", 8000, None),
        ];
        let rows = menu_rows(&models, &providers);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["p1::a", "p2::c"]
        );
        assert_eq!(rows[1].detail, "乙 · 16K 上下文 · 10ms");
    }

    #[test]
    fn menu_height_is_capped_and_has_room_for_the_empty_hint() {
        assert_eq!(menu_size(0), (272.0, 62.0));
        assert_eq!(menu_size(2), (272.0, 98.0));
        assert_eq!(
            menu_size(50),
            (272.0, 322.0),
            "最高 320 + 边框，超出部分在框内滚动"
        );
    }
}
