//! tokio（engine）↔ GPUI（前台）桥接
//!
//! engine 的 `ChatEngine::send_message` 是 `Send + 'static` 的 tokio future；GPUI 视图是
//! 单线程的 `Entity`（不能被 tokio 任务捕获 §3.3）。两边只经 channel 交互：
//!
//! ```text
//! tokio 线程池                          GPUI 前台
//! send_message ──StreamEvent──► rx.recv().await ─批量─► on_batch(view, events)
//!      ▲                                                   │
//!      └──── stop_generation / approve / answer ◄───────────┘（直接调 Arc<ChatEngine>）
//! ```
//!
//! # ⚠️ 为什么不用 `gpui_tokio::Tokio::spawn`
//!
//! 它返回的 GPUI `Task` **被 drop 时会 abort 底层 tokio 任务**（`gpui_tokio.rs` 的 `defer(abort)`）。
//! 若把对话任务交给视图持有，视图销毁 / 窗口关闭即中止生成：
//! - Esc / 点击外部关闭**不得**中断流式；
//! - `send_message` 在中途被杀，来不及 `release_generation` 与持久化 →
//!   此后每次发送都报「已有生成任务正在进行中」，且本轮消息丢失。
//!
//! 因此对话任务用 `Tokio::handle(cx).spawn(..)` 启动：其 `JoinHandle` 被 drop 只是分离，不会中止。
//! 停止生成的**唯一**途径是 `ChatEngine::stop_generation`（走完持久化与终态事件）。
//!
//! # ⚠️ engine 的 async 方法必须在 tokio 上执行
//!
//! `ChatEngine` 的 `get_config` / `load_messages` / `save_config` 等内部用 `tokio::task::spawn_blocking`
//! （v1 的 `run_blocking`）。在 GPUI 前台直接 `.await` 会 panic：
//! `there is no reactor running, must be called from the context of a Tokio 1.x runtime`（实测）。
//! 一律经 [`spawn_engine`] 调用。同步方法（`stop_generation` / `approve_tool_call` /
//! `answer_tool_question`）可直接调用。

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::Message;
use buddy_engine::streaming::{StreamEvent, StreamEventEmitter};
use gpui::{App, AppContext as _, AsyncApp, Context, Task, WeakEntity};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc::error::TryRecvError;

/// 初始化 tokio 运行时（`gpui_tokio`）。必须在第一次 [`start_chat`] 之前调用一次。
pub fn init(cx: &mut App) {
    gpui_tokio::init(cx);
}

/// 在 tokio 上执行一个 engine future，结果以 GPUI `Task` 交回前台。
///
/// 与 `gpui_tokio::Tokio::spawn` 不同，丢弃返回的 `Task` **不会**中止 tokio 上的执行
/// （避免保存配置、写消息等操作被 UI 生命周期截断）。
pub fn spawn_engine<R, Fut>(cx: &App, fut: Fut) -> Task<R>
where
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let join = gpui_tokio::Tokio::handle(cx).spawn(fut);
    cx.background_spawn(async move {
        match join.await {
            Ok(value) => value,
            // 从不 abort，JoinError 只可能来自 panic：原样向上传播
            Err(error) => std::panic::resume_unwind(error.into_panic()),
        }
    })
}

/// 一次对话在前台的结局
#[derive(Debug)]
pub struct ChatFinished {
    /// `send_message` 的返回值：`Err` 表示在占用生成通道之前就被拒绝（此时没有任何事件）
    pub result: Result<(), String>,
    /// 前台共收到的事件数
    pub events: usize,
    /// 前台回调次数（每次回调对应一次批量交付；远小于 `events` 说明合并生效）
    pub batches: usize,
}

/// 在 tokio 上发起一次对话，并把事件**按批**交付给视图 `T`。
///
/// - 每次 `rx.recv().await` 醒来后，把此刻已到达的事件全部取出，只回调 `on_batch` 一次
///   → 一批事件只触发一次视图更新（`cx.notify()` 由 `on_batch` 自行决定）。
/// - 视图已销毁时停止交付，但**不中止**对话：engine 照常完成、持久化并释放生成占用。
/// - 返回的 `Task` 在对话结束（事件通道关闭）后完成；丢弃它只停止前台交付，不影响生成。
pub fn start_chat<T: 'static>(
    engine: Arc<ChatEngine>,
    current: Message,
    model_id: String,
    cx: &mut Context<T>,
    on_batch: impl Fn(&mut T, Vec<StreamEvent>, &mut Context<T>) + 'static,
) -> Task<ChatFinished> {
    let (emitter, mut rx) = StreamEventEmitter::channel();
    // 分离式 spawn：JoinHandle 被 drop 不会 abort（见模块文档）
    let join = gpui_tokio::Tokio::handle(cx).spawn(async move {
        engine
            .send_message_from_history(emitter, current, model_id)
            .await
    });

    cx.spawn(async move |this: WeakEntity<T>, cx: &mut AsyncApp| {
        let (mut events, mut batches) = (0usize, 0usize);
        let mut view_alive = true;
        while let Some(first) = rx.recv().await {
            let mut batch = vec![first];
            loop {
                match rx.try_recv() {
                    Ok(ev) => batch.push(ev),
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                }
            }
            events += batch.len();
            if view_alive {
                batches += 1;
                view_alive = this
                    .update(cx, |view, cx| on_batch(view, batch, cx))
                    .is_ok();
            }
            // 视图已销毁：继续把通道读空直到关闭，只是不再交付
        }
        let result = match join.await {
            Ok(result) => result,
            Err(error) => Err(format!("对话任务异常结束：{error}")),
        };
        ChatFinished {
            result,
            events,
            batches,
        }
    })
}
