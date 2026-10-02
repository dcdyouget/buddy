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
//! | 模型按钮 | [`super::model_menu`] 在按钮上方打开菜单；选择后保存配置（串行） |
//! | 设置 | 叠加层（[`PageState::base_page`] 保持底层页不卸载）；设置本体为 `crate::settings::SettingsView`，侧滑层退出即释放输入 |
//!
//! **页面切换不改变窗口尺寸**：路由器不接触窗口，只发出 [`RouterEvent::PageChanged`]，
//! 「离开紧凑页时展开一次」由 Phase 07 依 [`expands_window`] 执行。

use super::approval_panel::Decision;
use super::chat_page::{ChatPage, ToolActions};
use super::composer::{Composer, ComposerEvent};
use super::empty_page::{EmptyPage, EmptyPageEvent};
use super::model_menu::{ModelMenu, menu_rows, open_model_menu};
use super::no_key_page::{NoKeyPage, NoKeyPageEvent};
use super::page_state::{EmptySend, Page, PageState, classify_empty_send, has_valid_config};
use super::session::{Conversation, HistoryLoader};
use super::state::{HISTORY_PAGE_SIZE, user_message_with_images};
use crate::chat_bridge::{self, spawn_engine};
use crate::theme_system::BuddyTheme;
use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, ImageAttachment, Message};
use gpui::{
    App, Context, Entity, EventEmitter, Focusable, SharedString, Subscription, Task, Window,
    WindowHandle, div, prelude::*,
};
use std::cell::RefCell;
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
    let history = engine
        .load_messages(offset, HISTORY_PAGE_SIZE)
        .await
        .unwrap_or_else(|e| {
            log::warn!("读取历史消息失败：{e}");
            Vec::new()
        });
    Loaded {
        config,
        history,
        offset,
    }
}

fn history_loader(engine: Arc<ChatEngine>) -> HistoryLoader {
    Rc::new(move |offset, limit, cx: &mut App| {
        let engine = engine.clone();
        spawn_engine(cx, async move { engine.load_messages(offset, limit).await })
    })
}

/// 工具交互回调：审批与 ask_user 回答经 engine 按调用 id 配对回传，成功后关闭对应界面状态
fn tool_actions(engine: &Arc<ChatEngine>, conversation: &Entity<Conversation>) -> ToolActions {
    let (approve_engine, approve_conversation) = (engine.clone(), conversation.clone());
    let (answer_engine, answer_conversation) = (engine.clone(), conversation.clone());
    let download_engine = engine.clone();
    ToolActions {
        decide: Rc::new(move |id: &str, decision: Decision, cx: &mut App| {
            let (approved, approve_all) = decision.flags();
            // v1 `resolveApproval` 后无论结果都 `dismiss`；engine 找不到对应槽位（已取消 / 超时）时也只记日志
            if let Err(error) = approve_engine.approve_tool_call(id, approved, approve_all) {
                log::warn!("审批回传失败：{error}");
            }
            approve_conversation.update(cx, |c, cx| c.clear_approval(cx));
        }),
        answer: Rc::new(
            move |id: &str, answer: super::ask_card::Answer, cx: &mut App| {
                let inputs = (!answer.inputs.is_empty()).then_some(answer.inputs);
                answer_engine.answer_tool_question(id, answer.selected, inputs, answer.custom)?;
                answer_conversation.update(cx, |c, cx| c.clear_question(cx));
                Ok(())
            },
        ),
        download: Rc::new(move |image: ImageAttachment, cx: &mut App| {
            let engine = download_engine.clone();
            spawn_engine(
                cx,
                async move { engine.download_generated_image(image).await },
            )
        }),
    }
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
    settings: Entity<crate::settings::SettingsView>,
    settings_motion: crate::settings::panel::SlideMotion,
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
    /// 模型菜单窗口（打开时）
    model_menu: Option<WindowHandle<ModelMenu>>,
    /// 配置保存的串行队列（v1 `configUpdateQueue`：快速连选两个模型时，第二次保存不能被第一次覆盖）
    config_save: Option<Task<()>>,
    config_save_state: Rc<RefCell<config_save::ConfigSaveState>>,
    /// 外壳注入系统热键事务；独立页面预览没有原生注册。
    hotkey_updater: Option<Rc<dyn Fn(&str) -> Result<(), String>>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<RouterEvent> for PageRouter {}

impl PageRouter {
    /// 新建（数据由 [`preload`] 取得）
    pub fn new(
        engine: Arc<ChatEngine>,
        loaded: Loaded,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let Loaded {
            config,
            history,
            offset,
        } = loaded;
        crate::theme_system::set_appearance(config.theme.clone().into(), cx);
        let conversation = cx.new(|_| {
            Conversation::with_history_page(history, offset, history_loader(engine.clone()))
        });
        let composer = cx.new(|cx| Composer::new_with_engine(Some(engine.clone()), window, cx));
        composer.update(cx, |c, cx| c.set_standalone(true, cx));
        let empty = cx.new(|_| EmptyPage::new(composer.clone()));
        let no_key = cx.new(NoKeyPage::new);
        let actions = tool_actions(&engine, &conversation);
        let chat = cx.new(|cx| ChatPage::new(conversation.clone(), composer.clone(), actions, cx));
        let settings =
            cx.new(|cx| crate::settings::SettingsView::new(config.clone(), engine.clone(), cx));
        let subscriptions = vec![
            cx.subscribe(
                &settings,
                |this, _, event: &crate::settings::SettingsEvent, cx| match event {
                    crate::settings::SettingsEvent::Back => this.close_settings(cx),
                    crate::settings::SettingsEvent::AddProvider(submission) => {
                        this.save_provider(submission.clone(), cx)
                    }
                    crate::settings::SettingsEvent::EditModel(edit) => {
                        this.save_model_edit(edit.clone(), cx)
                    }
                    crate::settings::SettingsEvent::HotkeyChanged(value) => {
                        this.save_preference(preferences::Preference::Hotkey(value.clone()), cx)
                    }
                    crate::settings::SettingsEvent::ThemeChanged(value) => {
                        this.save_preference(preferences::Preference::Theme(value.clone()), cx)
                    }
                },
            ),
            cx.subscribe_in(
                &composer,
                window,
                |this, _, event: &ComposerEvent, window, cx| match event {
                    ComposerEvent::Send(text) => this.send(text.clone(), cx),
                    ComposerEvent::Stop => this.engine.stop_generation(),
                    ComposerEvent::OpenSettings => this.open_settings(cx),
                    ComposerEvent::PickModel => this.toggle_model_menu(window, cx),
                },
            ),
            cx.subscribe(&empty, |this, _, event: &EmptyPageEvent, cx| match event {
                EmptyPageEvent::Expand => this.transition(cx, |p| p.set_page(Page::Conversation)),
                EmptyPageEvent::DismissError => {
                    this.conversation.update(cx, |c, cx| c.dismiss_error(cx))
                }
            }),
            cx.subscribe(&no_key, |this, _, NoKeyPageEvent::OpenSettings, cx| {
                this.open_settings(cx)
            }),
            cx.observe_in(&conversation, window, |this, _, window, cx| {
                this.conversation_changed(window, cx)
            }),
            // v1：窗口失焦 / 隐藏时立即放出缓冲，重新聚焦后追赶再恢复逐字
            cx.observe_window_activation(window, |this, window, cx| {
                let active = window.is_window_active();
                this.conversation.update(cx, |c, cx| {
                    if active {
                        c.window_shown(cx)
                    } else {
                        c.window_hidden(cx)
                    }
                });
            }),
        ];
        let mut this = Self {
            engine,
            config_save_state: Rc::new(RefCell::new(config_save::ConfigSaveState::new(
                config.clone(),
            ))),
            config,
            hotkey_updater: None,
            settings,
            settings_motion: crate::settings::panel::SlideMotion::default(),
            pages: PageState::new(),
            conversation,
            composer,
            empty,
            no_key,
            chat,
            was_streaming: false,
            focus_composer: true,
            run: None,
            model_menu: None,
            config_save: None,
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

    /// 生产外壳在保存热键前更新系统注册，写盘失败时恢复旧键。
    pub fn set_hotkey_updater(&mut self, update: Rc<dyn Fn(&str) -> Result<(), String>>) {
        self.hotkey_updater = Some(update);
    }

    /// 外部选中文本只写入当前 v1 允许的输入页，不触发发送。
    pub fn accept_selected_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        if !text.is_empty()
            && matches!(
                self.page(),
                Page::Empty | Page::NoApiKey | Page::Conversation
            )
        {
            self.composer
                .update(cx, |composer, cx| composer.set_draft(text, cx));
        }
    }

    /// 显隐通知直接更新缓冲；不依赖隐藏窗口的绘制或系统焦点回调。
    pub fn window_visibility_changed(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.conversation.update(cx, |conversation, cx| {
            if visible {
                conversation.window_shown(cx);
            } else {
                conversation.window_hidden(cx);
            }
        });
        self.focus_composer = visible && self.page() != Page::Settings;
        cx.notify();
    }

    /// 原生隐藏前关闭独立菜单并放出流式缓冲，保留整个会话。
    pub fn prepare_window_hide(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.model_menu.take() {
            let _ = menu.update(cx, |menu, window, cx| menu.close(window, cx));
        }
        self.window_visibility_changed(false, cx);
    }

    /// v1 闲置十分钟后呼出回紧凑页，历史和生成任务仍保留。
    pub fn invoked_after_idle(&mut self, cx: &mut Context<Self>) {
        self.transition(cx, |pages| pages.set_page(Page::Empty));
    }

    /// 对话页（自检用）
    pub fn chat_page(&self) -> &Entity<ChatPage> {
        &self.chat
    }

    /// 消息列表（自检用）
    pub fn transcript(&self, cx: &App) -> Entity<super::transcript::Transcript> {
        self.chat.read(cx).transcript().clone()
    }

    /// 会话（自检用）
    pub fn conversation(&self) -> &Entity<Conversation> {
        &self.conversation
    }

    /// 输入区（自检用）
    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    /// 无排队写入时替换外部配置并执行页面副作用；产品内编辑须走配置保存队列。
    pub fn set_config(&mut self, config: AppConfig, cx: &mut Context<Self>) {
        self.config_save_state.borrow_mut().reset(config.clone());
        self.publish_config(config, cx);
    }

    fn publish_config(&mut self, config: AppConfig, cx: &mut Context<Self>) {
        if cx.buddy_theme().appearance != config.theme.clone().into() {
            crate::theme_system::set_appearance(config.theme.clone().into(), cx);
        }
        self.config = config;
        self.settings
            .update(cx, |view, cx| view.set_config(self.config.clone(), cx));
        // 无效配置由 `apply_config` 处理（内容页退回空态）；有效时才可能从「无 Key」页补齐进入对话
        self.apply_config(cx);
        if self.valid_config() {
            self.transition(cx, |p| p.config_changed(true));
        }
    }

    /// 设置覆盖层实体（操作自检用）。
    pub fn settings_view(&self) -> &Entity<crate::settings::SettingsView> {
        &self.settings
    }

    /// 退出动画的绘制进度（自检用）；关闭后不再接受输入。
    pub fn settings_present(&self) -> bool {
        self.settings_motion.present()
    }

    /// 覆盖层当前可见量（真实帧动画自检用）。
    pub fn settings_amount(&self) -> f32 {
        self.settings_motion.amount()
    }

    /// 模型菜单窗口（自检用）
    pub fn model_menu(&self) -> Option<WindowHandle<ModelMenu>> {
        self.model_menu
    }

    /// 点模型按钮：菜单开着则关闭，否则在按钮上方打开（流式中按钮不存在，不会走到这里）
    fn toggle_model_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.model_menu.take()
            && open
                .update(cx, |menu, window, cx| menu.close(window, cx))
                .is_ok()
        {
            return;
        }
        if self.conversation.read(cx).state.is_streaming() {
            return;
        }
        let Some(anchor) = self.composer.read(cx).model_button_bounds() else {
            return;
        };
        let rows = menu_rows(&self.config.models, &self.config.providers);
        let router = cx.entity().downgrade();
        let on_select = std::rc::Rc::new(move |id: String, cx: &mut App| {
            let _ = router.update(cx, |router, cx| router.select_model(id, cx));
        });
        self.model_menu = open_model_menu(
            window,
            window.window_handle(),
            anchor,
            rows,
            self.config.selected_model_id.clone(),
            on_select,
            cx,
        );
    }

    /// 选择默认模型（v1 `setDefaultModel`）：内存立即生效，写盘串行进行；保存失败在错误条提示
    pub fn select_model(&mut self, id: String, cx: &mut Context<Self>) {
        if id == self.config.selected_model_id {
            return;
        }
        let mut config = self.config.clone();
        config.selected_model_id = id.clone();
        self.publish_config(config, cx);
        self.save_model_selection(id, cx);
    }

    /// 等待进行中的配置保存完成（自检用）
    pub fn take_config_save(&mut self) -> Option<Task<()>> {
        self.config_save.take()
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
        self.config
            .models
            .iter()
            .find(|m| m.id == self.config.selected_model_id)
    }

    /// 配置 → 输入区（是否支持图片）；无效配置时按 v1 退回空态
    fn apply_config(&mut self, cx: &mut Context<Self>) {
        let vision = self.selected_model().is_some_and(|m| m.supports_vision);
        self.composer
            .update(cx, |c, cx| c.set_supports_vision(vision, cx));
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
        if (from == Page::Settings) != (to == Page::Settings) {
            let shown = to == Page::Settings;
            self.settings_motion.set_shown(shown);
            self.settings
                .update(cx, |view, cx| view.set_active(shown, cx));
        }
        let base = self.pages.base_page();
        self.composer
            .update(cx, |c, cx| c.set_standalone(base == Page::Empty, cx));
        self.focus_composer = matches!(base, Page::Empty | Page::Conversation | Page::Streaming)
            && to != Page::Settings;
        cx.emit(RouterEvent::PageChanged { from, to });
        cx.notify();
    }

    /// 输入区发送（v1 `EmptyPage.handleSend` / `ChatPage.handleSend`）
    fn send(&mut self, text: String, cx: &mut Context<Self>) {
        if self.conversation.read(cx).state.is_streaming() {
            return;
        }
        let has_content = !text.trim().is_empty();
        let images = self.composer.read(cx).images();
        let image_count = images.len();
        if self.pages.base_page() == Page::Empty {
            let vision = self.selected_model().is_some_and(|m| m.supports_vision);
            match classify_empty_send(has_content, self.valid_config(), image_count > 0, vision) {
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
                    self.start(text, images, cx);
                    self.transition(cx, |p| p.set_page(Page::Streaming));
                }
            }
        } else if (has_content || image_count > 0) && !self.config.selected_model_id.is_empty() {
            self.start(text, images, cx);
        }
    }

    /// 发起对话（v1 `sendMessage`）：发给 engine 的是「已载入的历史 + 本条用户消息」
    fn start(
        &mut self,
        text: String,
        images: Vec<buddy_engine::models::ImageAttachment>,
        cx: &mut Context<Self>,
    ) {
        let model_id = self.config.selected_model_id.clone();
        self.composer.update(cx, |c, cx| {
            c.set_draft("", cx);
            let _ = c.take_images(cx);
        });
        let messages = self.conversation.update(cx, |c, cx| {
            c.begin_send(user_message_with_images(&text, images), &model_id, cx);
            let all = &c.state.messages;
            all[..all.len() - 1].to_vec()
        });
        let conversation = self.conversation.clone();
        let task = chat_bridge::start_chat(
            self.engine.clone(),
            messages,
            model_id,
            cx,
            move |_, events, cx| {
                conversation.update(cx, |c, cx| c.apply_events(events, cx));
            },
        );
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
            (
                state.is_streaming(),
                state.error.clone(),
                state.live.as_ref().map(|l| l.model_id.clone()),
            )
        };
        if streaming != self.was_streaming {
            let label = model_id.and_then(|id| {
                self.config
                    .models
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| SharedString::from(m.display_name.clone()))
            });
            self.composer
                .update(cx, |c, cx| c.set_streaming(streaming, label, window, cx));
        }
        self.empty.update(cx, |p, cx| p.set_error(error, cx));
        if self.was_streaming && !streaming {
            let needs_key = self.conversation.update(cx, |c, _| c.take_needs_api_key());
            self.transition(cx, |p| p.stream_finished(needs_key));
        }
        self.was_streaming = streaming;
    }
}

#[path = "router_config_save.rs"]
mod config_save;
#[path = "router_settings.rs"]
mod settings_save;

#[path = "router_preferences.rs"]
mod preferences;

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
        let settings = self.settings_motion.present().then(|| {
            let amount = self.settings_motion.amount();
            if self.settings_motion.animating() {
                window.request_animation_frame();
            }
            div()
                .absolute()
                .top_0()
                .left(gpui::relative(1.0 - amount))
                .size_full()
                .opacity(amount)
                .child(self.settings.clone())
                .into_any_element()
        });
        div().size_full().relative().child(base).children(settings)
    }
}
