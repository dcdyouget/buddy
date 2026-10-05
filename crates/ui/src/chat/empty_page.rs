//! 空态页—— 对应 v1 `src/pages/EmptyPage.tsx` 与 `.empty-shell` / `.empty-expand-trigger`
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 透明外壳（不再叠第二层玻璃）、内容贴底 | 根节点无底色，纵向 `justify_end` |
//! | 输入区 `hideBorder` + `disableAutoResize` → 独立气泡 | [`Composer::set_standalone`]（路由器在切页时设置） |
//! | 顶部居中的「展开」按钮：24×20、圆角 full、`--border-subtle`、`--control-surface`、`--shadow-static`、`ChevronUp` 14；悬停品牌色字 + `--composer-surface` 底；title「展开对话」 | 同，发出 [`EmptyPageEvent::Expand`] |
//! | 有错误时输入区上方显示 `.chat-error` | [`EmptyPage::set_error`] + [`error_banner`]，关闭发出 [`EmptyPageEvent::DismissError`] |
//! | 顶部左右两块拖拽区（`.empty-drag-region`） | 按 v1 留出中间展开按钮 真实拖拽 |
//!
//! 发送 / 模型选择 / 设置入口来自输入区自身的事件（[`EmptyPage::composer`]），由页面状态机订阅。

use super::composer::Composer;
use super::drag::{self, DragSource};
use super::message_row::error_banner;
use crate::components::TextTooltip;
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, box_shadows, tokens::metrics as m};
use gpui::{Context, Entity, EventEmitter, MouseButton, SharedString, Window, div, prelude::*, px};

/// 展开按钮尺寸（v1 `--space-6` × `--space-5`）
const EXPAND_WIDTH: f32 = m::SPACE_6;
const EXPAND_HEIGHT: f32 = m::SPACE_5;
/// 展开按钮距顶（v1 `top: 2px`）
const EXPAND_TOP: f32 = 2.0;

/// 空态页事件
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyPageEvent {
    /// 点击顶部「展开」（v1 `setPage('conversation')`）
    Expand,
    /// 关闭错误提示（v1 `setError(null)`）
    DismissError,
}

/// 空态页
pub struct EmptyPage {
    composer: Entity<Composer>,
    error: Option<SharedString>,
    /// 鼠标是否在「展开」按钮上（图标颜色与底色按此显式设置：`svg` 取自身文字样式，不跟随父元素的 `hover` 样式）
    expand_hovered: bool,
    drag_source: DragSource,
}

impl EventEmitter<EmptyPageEvent> for EmptyPage {}

impl EmptyPage {
    /// 新建。输入区由外部提供并与对话页共用同一个实体（v1 草稿存在 `chatStore`，两页共享）；
    /// 是否为独立气泡由路由器在切页时设置（[`Composer::set_standalone`]）
    pub fn new(composer: Entity<Composer>) -> Self {
        Self { composer, error: None, expand_hovered: false, drag_source: drag::default_drag_source() }
    }

    /// 输入区实体（发送 / 模型选择 / 设置事件由它发出）
    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    /// Replace the window-drag callback (used by shell self-tests).
    pub fn set_drag_source(&mut self, source: DragSource) {
        self.drag_source = source;
    }

    /// 设置 / 清除错误提示
    pub fn set_error(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.error = error.map(SharedString::from);
        cx.notify();
    }

    /// 「展开」按钮是否处于悬停态（自检用）
    pub fn expand_hovered(&self) -> bool {
        self.expand_hovered
    }

    /// 当前错误提示
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl Render for EmptyPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let c = theme.colors;
        let drag_source = self.drag_source.clone();
        self.composer.update(cx, |composer, _| {
            composer.set_drag_source(drag_source.clone());
        });
        let composer = self.composer.clone();
        let allow_blank_drag = self.error.is_none();
        let expand_bounds = std::rc::Rc::new(std::cell::Cell::new(None::<gpui::Bounds<gpui::Pixels>>));
        let expand_bounds_for_hit = expand_bounds.clone();
        // v1 `.empty-expand-trigger:hover`：品牌色文字 + `--composer-surface` 底
        let (fg, bg) = if self.expand_hovered { (c.buddy_primary, c.composer_surface) } else { (c.text_muted, c.control_surface) };
        div()
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                // The root owns only the remaining blank canvas. The Composer
                // records its actual window bounds so text, buttons, and its
                // padding never become a page-level drag target.
                if !allow_blank_drag
                    || expand_bounds_for_hit.get().is_some_and(|bounds| bounds.contains(&event.position))
                    || composer
                        .read(cx)
                        .bounds()
                        .is_some_and(|bounds| bounds.contains(&event.position))
                {
                    return;
                }
                cx.stop_propagation();
                drag::invoke(&drag_source, window);
            })
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .justify_end()
            .overflow_hidden()
            .children([
                drag::region(&self.drag_source)
                    .absolute()
                    .top_0()
                    .left(px(m::SPACE_2))
                    .right(gpui::relative(0.5))
                    .mr(px(m::SPACE_5))
                    .h(px(m::SPACE_3)),
                drag::region(&self.drag_source)
                    .absolute()
                    .top_0()
                    .left(gpui::relative(0.5))
                    .ml(px(m::SPACE_5))
                    .right(px(m::SPACE_2))
                    .h(px(m::SPACE_3)),
                drag::region(&self.drag_source)
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(px(m::SPACE_2)),
                drag::region(&self.drag_source)
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(m::SPACE_2)),
            ])
            .when_some(self.error.clone(), |d, message| {
                let this = cx.entity().downgrade();
                d.child(error_banner(
                    &message,
                    move |_, cx| {
                        let _ = this.update(cx, |_, cx| cx.emit(EmptyPageEvent::DismissError));
                    },
                    cx,
                ))
            })
            .child(self.composer.clone())
            // 展开按钮盖在输入区之上（v1 `z-index: 2`）：必须排在输入区之后绘制
            .child(
                div()
                    .id("empty-expand")
                    .absolute()
                    .top(px(EXPAND_TOP))
                    .left(gpui::relative(0.5))
                    .ml(px(-EXPAND_WIDTH / 2.0))
                    .w(px(EXPAND_WIDTH))
                    .h(px(EXPAND_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(c.border_subtle)
                    .rounded(px(m::RADIUS_FULL))
                    .text_color(fg)
                    .bg(bg)
                    .shadow(box_shadows(theme.shadows.shadow_static))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.expand_hovered = *hovered;
                        cx.notify();
                    }))
                    .tooltip(|_, cx| TextTooltip::view("展开对话", cx))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(EmptyPageEvent::Expand)))
                    .child(icon(IconName::ChevronUp, px(14.0)).text_color(fg))
                    .child(gpui::canvas(move |bounds, _, _| expand_bounds.set(Some(bounds)), |_, _, _, _| {})
                        .absolute().top_0().left_0().size_full()),
            )
    }
}
