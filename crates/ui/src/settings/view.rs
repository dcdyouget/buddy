//! SettingsPage 编排与布局。子项的编辑交互由对应 S06 spec 接入，未实现部分只读显示。
//!
//! 设置层始终复用同一个实体；退出绘制不再注册鼠标 / 键盘处理，避免透明层吞掉底层点击。

use super::{
    panel::SlideMotion,
    provider::{AddProviderPanel, ProviderEvent},
    provider_form::ProviderSubmission,
};
use crate::{
    icons::{IconName, icon},
    theme_system::{BuddyTheme, tokens::metrics as m},
};
use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Theme};
use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Pixels,
    Render, ScrollHandle, Subscription, Window, canvas, div, prelude::*, px,
};
use std::{cell::Cell, rc::Rc, sync::Arc};

/// 设置页向路由器发出的事件。
#[derive(Clone, Debug)]
pub enum SettingsEvent {
    /// 返回设置前的页面。
    Back,
    /// 完整提交交给 Router 串行保存，成功后才发布内存配置。
    AddProvider(ProviderSubmission),
}

/// 作为原页面上的全尺寸覆盖层，保留底层输入草稿和生成任务。
pub struct SettingsView {
    config: AppConfig,
    back: FocusHandle,
    active: bool,
    focus_pending: bool,
    scroll: ScrollHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    add: FocusHandle,
    add_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    provider: gpui::Entity<AddProviderPanel>,
    provider_motion: SlideMotion,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.back.clone()
    }
}

impl SettingsView {
    /// 构建设置覆盖层。
    pub fn new(config: AppConfig, engine: Arc<ChatEngine>, cx: &mut Context<Self>) -> Self {
        let provider = cx.new(|cx| AddProviderPanel::new(engine, cx));
        let subscriptions =
            vec![cx.subscribe(
                &provider,
                |this, _, event: &ProviderEvent, cx| match event {
                    ProviderEvent::Back => this.close_provider(cx),
                    ProviderEvent::Submit(submission)
                        if this.active && this.provider_motion.interactive() =>
                    {
                        cx.emit(SettingsEvent::AddProvider(submission.clone()));
                    }
                    ProviderEvent::Submit(_) => {}
                },
            )];
        Self {
            config,
            back: cx.focus_handle(),
            active: false,
            focus_pending: false,
            scroll: ScrollHandle::new(),
            bounds: Rc::new(Cell::new(None)),
            add: cx.focus_handle(),
            add_bounds: Rc::new(Cell::new(None)),
            provider,
            provider_motion: SlideMotion::default(),
            _subscriptions: subscriptions,
        }
    }

    /// 路由器外部变更配置时同步显示。
    pub fn set_config(&mut self, config: AppConfig, cx: &mut Context<Self>) {
        self.config = config;
        cx.notify();
    }

    /// 打开时将焦点移到返回按钮；关闭立即释放事件，退出动画仅绘制。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        self.focus_pending = active;
        if !active {
            self.close_provider(cx);
        }
        cx.notify();
    }

    /// 二层添加 Provider 面板（真实输入自检用）。
    pub fn provider_panel(&self) -> &gpui::Entity<AddProviderPanel> {
        &self.provider
    }

    /// 是否正在显示添加 Provider 流程。
    pub fn provider_open(&self) -> bool {
        self.provider_motion.interactive()
    }

    /// 最后一帧添加按钮边界。
    pub fn add_button_bounds(&self) -> Option<Bounds<Pixels>> {
        self.add_bounds.get()
    }

    /// 在当前设置页打开新增面板；每次打开与 v1 挂载新表单一致。
    pub fn open_provider(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.provider_motion.interactive() {
            return;
        }
        self.provider.update(cx, |panel, cx| {
            panel.reset(cx);
            panel.set_active(true, cx);
        });
        self.provider_motion.set_shown(true);
        cx.notify();
    }

    fn close_provider(&mut self, cx: &mut Context<Self>) {
        self.provider_motion.set_shown(false);
        self.provider
            .update(cx, |panel, cx| panel.set_active(false, cx));
        self.focus_pending = self.active;
        cx.notify();
    }

    /// 内容滚动位置（真实滚轮自检）。
    pub fn scroll_offset(&self) -> gpui::Point<gpui::Pixels> {
        self.scroll.offset()
    }

    /// 是否接受输入。
    pub fn active(&self) -> bool {
        self.active
    }

    /// 最后一帧实际覆盖层边界（动画位移 / 布局自检用）。
    pub fn painted_bounds(&self) -> Option<Bounds<Pixels>> {
        self.bounds.get()
    }
}

#[path = "view/models.rs"]
mod models;
#[path = "view/render.rs"]
mod render;
