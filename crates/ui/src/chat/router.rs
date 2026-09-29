//! 页面路由器（S05-18）—— 对应 v1 `App.tsx` 的 `PageRenderer` 与其配置 / 流式副作用，并接入 engine
//!
//! 转换条件见 [`super::page_state`]；本模块负责把它们接到真实的输入：
//!
//! | 输入 | 处理 |
//! |------|------|
//! | 启动 | [`preload`] 读配置与最新一页历史（经 tokio），页面为 `empty`（不因已有历史自动进入对话） |
//! | 输入区发送 | 空态页按 [`classify_empty_send`] 判定；其余页需有默认模型。`send_message(历史 + 用户消息)` 经 [`chat_bridge::start_chat`] |
//! | engine 事件 | [`Conversation::apply_events`]；流式结束时按是否 401 落点（`noapikey` / `conversation`） |
//! | 界面生成的提示消息（配额 / 服务器 / 网络） | 取走 `pending_saves` → `save_message`（v1 `saveMessage`） |
//! | 历史分页 | [`Conversation::with_history_page`] + `load_messages`（读取前先把偏移换算为「最新一页」） |
//! | 设置 | 叠加层（[`PageState::base_page`] 保持底层页不卸载）；设置页本体归 S06-01，此处是占位 |
//!
//! **页面切换不改变窗口尺寸**：路由器不接触窗口，只发出 [`RouterEvent::PageChanged`]，
//! 「离开紧凑页时展开一次」由 Phase 07 依 [`expands_window`] 执行。

use super::chat_page::ChatPage;
use super::composer::{Composer, ComposerEvent};
use super::empty_page::{EmptyPage, EmptyPageEvent};
use super::no_key_page::{NoKeyPage, NoKeyPageEvent};
use super::page_state::{EmptySend, Page, PageState, classify_empty_send, has_valid_config};
use super::session::{Conversation, HistoryLoader};
use super::state::{HISTORY_PAGE_SIZE, user_message};
use crate::chat_bridge::{self, spawn_engine};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Message};
use gpui::{AnyElement, App, Context, Entity, EventEmitter, Focusable, FontWeight, SharedString, Subscription, Task, Window, div, prelude::*, px};
use std::rc::Rc;
use std::sync::Arc;

/// 启动时读到的数据
pub struct Loaded {
    /// 应用配置
    pub config: AppConfig,
    /// 最新一页历史
    pub history: Vec<Message>,
    /// 该页在全部历史中的起始偏移
    pub offset: u64,
}

/// 读配置与最新一页历史（v1 `loadConfig` + `loadMessages`）。**须在 tokio 上执行**（[`spawn_engine`]）；
/// 读取失败按空数据继续并记录日志，界面仍可用
pub async fn preload(engine: Arc<ChatEngine>) -> Loaded {
    let config = engine.get_config().await.unwrap_or_else(|e| {
        log::warn!("读取配置失败，按默认配置启动：{e}");
        AppConfig::default()
    });
    let count = engine.get_message_count().await.unwrap_or_else(|e| {
        log::warn!("读取消息数量失败：{e}");
        0
    });
    let offset = count.saturating_sub(HISTORY_PAGE_SIZE);
    let history = engine.load_messages(offset, HISTORY_PAGE_SIZE).await.unwrap_or_else(|e| {
        log::warn!("读取历史消息失败：{e}");
        Vec::new()
    });
    Loaded { config, history, offset }
}

fn history_loader(engine: Arc<ChatEngine>) -> HistoryLoader {
    Rc::new(move |offset, limit, cx: &mut App| {
        let engine = engine.clone();
        spawn_engine(cx, async move { engine.load_messages(offset, limit).await })
    })
}

/// 路由器事件
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouterEvent {
    /// 页面已切换。窗口壳据 [`super::page_state::expands_window`] 决定是否展开窗口
    PageChanged {
        /// 原页面
        from: Page,
        /// 新页面
        to: Page,
    },
}

/// 页面路由器
pub struct PageRouter {
    engine: Arc<ChatEngine>,
    config: AppConfig,
    pages: PageState,
    conversation: Entity<Conversation>,
    composer: Entity<Composer>,
    empty: Entity<EmptyPage>,
    no_key: Entity<NoKeyPage>,
    chat: Entity<ChatPage>,
    was_streaming: bool,
    /// 下一次渲染时把焦点交给输入区（切页后；切页可能发生在没有 `Window` 的上下文）
    focus_composer: bool,
    run: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<RouterEvent> for PageRouter {}

impl PageRouter {
    /// 新建（数据由 [`preload`] 取得）
    pub fn new(engine: Arc<ChatEngine>, loaded: Loaded, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Loaded { config, history, offset } = loaded;
        let conversation = cx.new(|_| Conversation::with_history_page(history, offset, history_loader(engine.clone())));
        let composer = cx.new(|cx| Composer::new(window, cx));
        composer.update(cx, |c, cx| c.set_standalone(true, cx));
        let empty = cx.new(|_| EmptyPage::new(composer.clone()));
        let no_key = cx.new(NoKeyPage::new);
        let chat = cx.new(|cx| ChatPage::new(conversation.clone(), composer.clone(), cx));
        let subscriptions = vec![
            cx.subscribe_in(&composer, window, |this, _, event: &ComposerEvent, _, cx| match event {
                ComposerEvent::Send(text) => this.send(text.clone(), cx),
                ComposerEvent::Stop => this.engine.stop_generation(),
                ComposerEvent::OpenSettings => this.open_settings(cx),
                // 模型选择器归 S05-15
                ComposerEvent::PickModel => {}
            }),
            cx.subscribe(&empty, |this, _, event: &EmptyPageEvent, cx| match event {
                EmptyPageEvent::Expand => this.transition(cx, |p| p.set_page(Page::Conversation)),
                EmptyPageEvent::DismissError => this.conversation.update(cx, |c, cx| c.dismiss_error(cx)),
            }),
            cx.subscribe(&no_key, |this, _, NoKeyPageEvent::OpenSettings, cx| this.open_settings(cx)),
            cx.observe_in(&conversation, window, |this, _, window, cx| this.conversation_changed(window, cx)),
            // v1：窗口失焦 / 隐藏时立即放出缓冲，重新聚焦后追赶再恢复逐字
            cx.observe_window_activation(window, |this, window, cx| {
                let active = window.is_window_active();
                this.conversation.update(cx, |c, cx| if active { c.window_shown(cx) } else { c.window_hidden(cx) });
            }),
        ];
        let mut this = Self {
            engine,
            config,
            pages: PageState::new(),
            conversation,
            composer,
            empty,
            no_key,
            chat,
            was_streaming: false,
            focus_composer: true,
            run: None,
            _subscriptions: subscriptions,
        };
        this.apply_config(cx);
        this
    }

    /// 当前页
    pub fn page(&self) -> Page {
        self.pages.current()
    }

    /// 实际渲染的底层页面（设置页叠在它之上）
    pub fn base_page(&self) -> Page {
        self.pages.base_page()
    }

    /// 当前配置
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// 会话（自检用）
    pub fn conversation(&self) -> &Entity<Conversation> {
        &self.conversation
    }

    /// 输入区（自检用）
    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    /// 配置变化（设置页保存后 / 外部补齐 Key）：更新配置并执行 v1 `App.tsx` 的配置副作用
    pub fn set_config(&mut self, config: AppConfig, cx: &mut Context<Self>) {
        self.config = config;
        // 无效配置由 `apply_config` 处理（内容页退回空态）；有效时才可能从「无 Key」页补齐进入对话
        self.apply_config(cx);
        if self.valid_config() {
            self.transition(cx, |p| p.config_changed(true));
        }
    }

    /// 打开设置（输入区按钮、无 Key 页、菜单栏「设置…」）
    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.transition(cx, |p| p.set_page(Page::Settings));
    }

    /// 关闭设置（v1 设置页 `onBack`）
    pub fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.transition(cx, |p| p.close_settings());
    }

    fn valid_config(&self) -> bool {
        has_valid_config(self.config.providers.len(), &self.config.selected_model_id)
    }

    fn selected_model(&self) -> Option<&buddy_engine::models::ModelInfo> {
        self.config.models.iter().find(|m| m.id == self.config.selected_model_id)
    }

    /// 配置 → 输入区（是否支持图片）；无效配置时按 v1 退回空态
    fn apply_config(&mut self, cx: &mut Context<Self>) {
        let vision = self.selected_model().is_some_and(|m| m.supports_vision);
        self.composer.update(cx, |c, cx| c.set_supports_vision(vision, cx));
        if !self.valid_config() {
            self.transition(cx, |p| p.config_changed(false));
        }
    }

    /// 执行一次页面转换；发生切换时同步输入区形态并通知
    fn transition(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut PageState) -> bool) {
        let from = self.pages.current();
        if !change(&mut self.pages) {
            return;
        }
        let to = self.pages.current();
        let base = self.pages.base_page();
        self.composer.update(cx, |c, cx| c.set_standalone(base == Page::Empty, cx));
        self.focus_composer = matches!(base, Page::Empty | Page::Conversation | Page::Streaming) && to != Page::Settings;
        cx.emit(RouterEvent::PageChanged { from, to });
        cx.notify();
    }

    /// 输入区发送（v1 `EmptyPage.handleSend` / `ChatPage.handleSend`）
    fn send(&mut self, text: String, cx: &mut Context<Self>) {
        if self.conversation.read(cx).state.is_streaming() {
            return;
        }
        let has_content = !text.trim().is_empty();
        if self.pages.base_page() == Page::Empty {
            let vision = self.selected_model().is_some_and(|m| m.supports_vision);
            match classify_empty_send(has_content, self.valid_config(), false, vision) {
                EmptySend::Ignore => {}
                // 草稿保留（不清空输入区），补好配置后回来还在
                EmptySend::NeedsKey => self.transition(cx, |p| p.set_page(Page::NoApiKey)),
                EmptySend::ImagesUnsupported => {
                    self.conversation.update(cx, |c, cx| {
                        c.state.error = Some("当前模型不支持图片，请移除图片或切换模型".into());
                        c.state.revision += 1;
                        cx.notify();
                    });
                }
                EmptySend::Send => {
                    self.start(text, cx);
                    self.transition(cx, |p| p.set_page(Page::Streaming));
                }
            }
        } else if has_content && !self.config.selected_model_id.is_empty() {
            self.start(text, cx);
        }
    }

    /// 发起对话（v1 `sendMessage`）：发给 engine 的是「已载入的历史 + 本条用户消息」
    fn start(&mut self, text: String, cx: &mut Context<Self>) {
        let model_id = self.config.selected_model_id.clone();
        self.composer.update(cx, |c, cx| c.set_draft("", cx));
        let messages = self.conversation.update(cx, |c, cx| {
            c.begin_send(user_message(&text), &model_id, cx);
            let all = &c.state.messages;
            all[..all.len() - 1].to_vec()
        });
        let conversation = self.conversation.clone();
        let task = chat_bridge::start_chat(self.engine.clone(), messages, model_id, cx, move |_, events, cx| {
            conversation.update(cx, |c, cx| c.apply_events(events, cx));
        });
        let conversation = self.conversation.clone();
        self.run = Some(cx.spawn(async move |_, cx| {
            let finished = task.await;
            // 在占用生成通道前就被拒绝（没有任何事件）：移除空占位并报错
            if let Err(message) = finished.result {
                let _ = conversation.update(cx, |c, cx| c.send_rejected(message, cx));
            }
        }));
    }

    /// 会话变化：落盘提示消息、同步输入区与错误条、流式结束时决定落点
    fn conversation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for message in self.conversation.update(cx, |c, _| c.take_pending_saves()) {
            let engine = self.engine.clone();
            spawn_engine(cx, async move { engine.save_message(message).await }).detach();
        }
        let (streaming, error, model_id) = {
            let state = &self.conversation.read(cx).state;
            (state.is_streaming(), state.error.clone(), state.live.as_ref().map(|l| l.model_id.clone()))
        };
        if streaming != self.was_streaming {
            let label = model_id.and_then(|id| self.config.models.iter().find(|m| m.id == id).map(|m| SharedString::from(m.display_name.clone())));
            self.composer.update(cx, |c, cx| c.set_streaming(streaming, label, window, cx));
        }
        self.empty.update(cx, |p, cx| p.set_error(error, cx));
        if self.was_streaming && !streaming {
            let needs_key = self.conversation.update(cx, |c, _| c.take_needs_api_key());
            self.transition(cx, |p| p.stream_finished(needs_key));
        }
        self.was_streaming = streaming;
    }

    /// 设置页占位（S06-01 实现设置页后替换）
    fn settings_placeholder(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.buddy_theme().colors;
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(m::SPACE_3))
            .rounded(px(m::RADIUS_XL))
            .border_1()
            .border_color(c.border_default)
            .bg(c.bg_surface)
            .child(div().text_color(c.text_primary).text_size(px(m::FONT_SIZE_LG)).font_weight(FontWeight(600.0)).child("设置"))
            .child(div().text_color(c.text_muted).text_size(px(m::FONT_SIZE_SM)).child("设置页尚未实现（S06-01）"))
            .child(
                div()
                    .id("settings-back")
                    .px(px(m::SPACE_4))
                    .py(px(m::SPACE_2))
                    .rounded(px(m::RADIUS_MD))
                    .border_1()
                    .border_color(c.border_default)
                    .text_color(c.text_primary)
                    .cursor_pointer()
                    .hover(|s| s.bg(c.control_surface))
                    .on_click(cx.listener(|this, _, _, cx| this.close_settings(cx)))
                    .child("返回"),
            )
            .into_any_element()
    }
}

impl Render for PageRouter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_composer) {
            window.focus(&self.composer.focus_handle(cx), cx);
        }
        let base = match self.pages.base_page() {
            Page::Empty | Page::Settings => self.empty.clone().into_any_element(),
            Page::NoApiKey => self.no_key.clone().into_any_element(),
            Page::Conversation | Page::Streaming => self.chat.clone().into_any_element(),
        };
        let settings = (self.pages.current() == Page::Settings).then(|| self.settings_placeholder(cx));
        div().size_full().relative().child(base).children(settings)
    }
}
