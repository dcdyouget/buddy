//! 空态页（S05-16）—— 对应 v1 `src/pages/EmptyPage.tsx` 与 `.empty-shell` / `.empty-expand-trigger`
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 透明外壳（不再叠第二层玻璃）、内容贴底 | 根节点无底色，纵向 `justify_end` |
//! | 输入区 `hideBorder` + `disableAutoResize` → 独立气泡 | [`Composer::set_standalone`]（路由器在切页时设置） |
//! | 顶部居中的「展开」按钮：24×20、圆角 full、`--border-subtle`、`--control-surface`、`--shadow-static`、`ChevronUp` 14；悬停品牌色字 + `--composer-surface` 底；title「展开对话」 | 同，发出 [`EmptyPageEvent::Expand`] |
//! | 有错误时输入区上方显示 `.chat-error` | [`EmptyPage::set_error`] + [`error_banner`]，关闭发出 [`EmptyPageEvent::DismissError`] |
//! | 顶部左右两块拖拽区（`.empty-drag-region`） | **不做**：窗口拖拽属 Phase 07 |
//!
//! 发送 / 模型选择 / 设置入口来自输入区自身的事件（[`EmptyPage::composer`]），由页面状态机（S05-18）订阅。

use super::composer::Composer;
use super::message_row::error_banner;
use crate::components::TextTooltip;
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, box_shadows, tokens::metrics as m};
use gpui::{Context, Entity, EventEmitter, SharedString, Window, div, prelude::*, px};

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
}

impl EventEmitter<EmptyPageEvent> for EmptyPage {}

impl EmptyPage {
    /// 新建。输入区由外部提供并与对话页共用同一个实体（v1 草稿存在 `chatStore`，两页共享）；
    /// 是否为独立气泡由路由器在切页时设置（[`Composer::set_standalone`]）
    pub fn new(composer: Entity<Composer>) -> Self {
        Self { composer, error: None }
    }

    /// 输入区实体（发送 / 模型选择 / 设置事件由它发出）
    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    /// 设置 / 清除错误提示
    pub fn set_error(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.error = error.map(SharedString::from);
        cx.notify();
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
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .justify_end()
            .overflow_hidden()
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
                    .text_color(c.text_muted)
                    .bg(c.control_surface)
                    .shadow(box_shadows(theme.shadows.shadow_static))
                    .cursor_pointer()
                    .hover(|s| s.text_color(c.buddy_primary).bg(c.composer_surface))
                    .tooltip(|_, cx| TextTooltip::view("展开对话", cx))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(EmptyPageEvent::Expand)))
                    .child(icon(IconName::ChevronUp, px(14.0))),
            )
    }
}
