//! SettingsPage 编排与布局。子项的编辑交互由对应 S06 spec 接入，未实现部分只读显示。
//!
//! 设置层始终复用同一个实体；退出绘制不再注册鼠标 / 键盘处理，避免透明层吞掉底层点击。

use super::model_list::ModelListView;
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
    /// 已保存模型的编辑交给 Router 串行保存。
    EditModel(crate::settings::model_config::ModelEdit),
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
    model_list: gpui::Entity<ModelListView>,
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
        let model_list = cx.new(|cx| ModelListView::new(config.clone(), cx));
        let subscriptions = vec![
            cx.subscribe(
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
            ),
            cx.subscribe(
                &model_list,
                |this, _, event: &crate::settings::model_config::ModelEdit, cx| {
                    if this.active && !this.provider_motion.interactive() {
                        cx.emit(SettingsEvent::EditModel(event.clone()));
                    }
                },
            ),
        ];
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
            model_list,
            provider_motion: SlideMotion::default(),
            _subscriptions: subscriptions,
        }
    }

    /// 路由器外部变更配置时同步显示。
    pub fn set_config(&mut self, config: AppConfig, cx: &mut Context<Self>) {
        self.config = config;
        self.model_list
            .update(cx, |models, cx| models.set_config(self.config.clone(), cx));
        cx.notify();
    }

    /// 打开时将焦点移到返回按钮；关闭立即释放事件，退出动画仅绘制。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        self.focus_pending = active;
        if !active {
            self.close_provider(cx);
        }
        self.model_list
            .update(cx, |models, cx| models.set_active(active, cx));
        cx.notify();
    }

    /// 配置保存期间禁用模型列表交互。
    pub fn set_model_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.model_list
            .update(cx, |models, cx| models.set_saving(saving, cx));
        cx.notify();
    }

    /// 保存失败时恢复交互并在列表底部显示错误。
    pub fn model_save_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.model_list
            .update(cx, |models, cx| models.model_save_failed(message, cx));
        cx.notify();
    }

    /// 获取已保存模型列表实体，供 Router 和真实输入自测访问。
    pub fn model_list(&self) -> &gpui::Entity<ModelListView> {
        &self.model_list
    }

    /// 获取设置内容滚动视口的当前边界，供焦点可见性自测使用。
    pub fn scroll_bounds(&self) -> Bounds<Pixels> {
        self.scroll.bounds()
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
        self.model_list
            .update(cx, |models, cx| models.set_active(false, cx));
        self.provider_motion.set_shown(true);
        cx.notify();
    }

    fn close_provider(&mut self, cx: &mut Context<Self>) {
        self.provider_motion.set_shown(false);
        self.provider
            .update(cx, |panel, cx| panel.set_active(false, cx));
        self.model_list
            .update(cx, |models, cx| models.set_active(self.active, cx));
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

    /// 把 Tab 导航选中的模型控件或添加入口滚入设置内容视口。
    pub(super) fn scroll_to_model_control(&self, id: &str, cx: &App) {
        let target = if id == "add" {
            self.add_bounds.get()
        } else {
            self.model_list.read(cx).control_bounds(id)
        };
        let Some(target) = target else {
            return;
        };
        let viewport = self.scroll.bounds();
        if viewport.size.height <= gpui::px(0.) {
            return;
        }
        let offset = self.scroll.offset();
        let mut next = offset;
        if target.top() < viewport.top() {
            next.y += viewport.top() - target.top();
        } else if target.bottom() > viewport.bottom() {
            next.y -= target.bottom() - viewport.bottom();
        }
        let max = self.scroll.max_offset();
        next.y = next.y.max(-max.y).min(gpui::px(0.));
        if next != offset {
            self.scroll.set_offset(next);
        }
    }
}

#[path = "view/models.rs"]
mod models;
#[path = "view/render.rs"]
mod render;
