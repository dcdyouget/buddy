//! 会话实体（S05-17）：持有 [`ChatState`]，接收 engine 事件批次，流式期间按帧推进节奏器
//!
//! 视图（Transcript、Composer……）各自 `observe` 本实体，只在 [`ChatState::revision`] 变化时收到通知；
//! 节奏器每帧推进但**没有新字放出时不通知** —— 流式中空转的帧不触发任何视图重算（v1 为每帧 set store）。

use super::state::ChatState;
use buddy_engine::models::Message;
use buddy_engine::streaming::StreamEvent;
use gpui::{AsyncApp, Context, Task, WeakEntity};
use std::time::{Duration, Instant};

/// 流式期间推进节奏器的间隔（约一帧）
const PUMP_INTERVAL: Duration = Duration::from_millis(16);

/// 会话
pub struct Conversation {
    /// 对话状态
    pub state: ChatState,
    started: Instant,
    pump: Option<Task<()>>,
}

impl Conversation {
    /// 以历史消息建立
    pub fn new(history: Vec<Message>) -> Self {
        Self { state: ChatState::from_history(history), started: Instant::now(), pump: None }
    }

    /// 单调时钟（毫秒），节奏器与落定效果共用
    pub fn now_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    /// 发送：追加用户消息并进入流式
    pub fn begin_send(&mut self, user: Message, model_id: &str, cx: &mut Context<Self>) {
        self.state.begin_send(user, model_id);
        self.ensure_pump(cx);
        cx.notify();
    }

    /// 应用一批 engine 事件（`chat_bridge::start_chat` 的回调）
    pub fn apply_events(&mut self, events: Vec<StreamEvent>, cx: &mut Context<Self>) {
        let revision = self.state.revision;
        let now = self.now_ms();
        for event in events {
            self.state.push_event(event, now);
        }
        self.ensure_pump(cx);
        if self.state.revision != revision {
            cx.notify();
        }
    }

    /// 窗口失焦 / 隐藏：立即放出（v1 同）
    pub fn window_hidden(&mut self, cx: &mut Context<Self>) {
        let revision = self.state.revision;
        let now = self.now_ms();
        self.state.hide(now);
        if self.state.revision != revision {
            cx.notify();
        }
    }

    /// 窗口重新聚焦
    pub fn window_shown(&mut self, cx: &mut Context<Self>) {
        let revision = self.state.revision;
        let now = self.now_ms();
        self.state.show(now);
        if self.state.revision != revision {
            cx.notify();
        }
    }

    /// 流式中保持一个按帧推进的任务；流式结束即退出
    fn ensure_pump(&mut self, cx: &mut Context<Self>) {
        if !self.state.is_streaming() || self.pump.is_some() {
            return;
        }
        self.pump = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(PUMP_INTERVAL).await;
                let keep = this.update(cx, |c, cx| {
                    let revision = c.state.revision;
                    let now = c.now_ms();
                    c.state.tick(now);
                    if c.state.revision != revision {
                        cx.notify();
                    }
                    let streaming = c.state.is_streaming();
                    if !streaming {
                        c.pump = None;
                    }
                    streaming
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        }));
    }
}
