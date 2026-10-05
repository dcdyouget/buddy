//! Buddy 引擎层（MIT）
//!
//! # 分层
//!
//! ```text
//! buddy-engine  ← 本 crate：零 GPUI / 零 Tauri / 零 GPL
//! buddy-ui      ← 依赖 GPUI + zed theme/ui（GPL-3.0-or-later）
//! buddy-app     ← 组装两者（GPL-3.0-or-later）
//! ```
//!
//! # 为什么引擎层必须保持纯净
//!
//! zed 的 `theme` / `ui` / vendored `markdown` 均为 **GPL-3.0-or-later**。
//! 一旦引擎层链接了它们，整个 engine 也变成 GPL，"以后闭源 engine" 的可能性消失。
//!
//! **S00-08 已实测验证**：engine 的依赖树里 0 处 GPUI / Tauri / zed crate。
//! 这不是设想，是已验证的边界。
//!
//! # 模块（S02-* 将逐一迁入）
//!
//! - `models` —— 数据模型（`S02-03`）
//! - `providers` —— LLM provider 适配（`S02-01`）
//! - `streaming` —— 流式事件（`S02-05`）
//! - `tools` —— 工具调用（`S02-02`）
//! - `storage` —— 持久化（`S02-04`）
//!
//! # 与 UI 的接口
//!
//! **没有 IPC 层**。UI 直接调用本 crate 的类型与方法
//! （v1 的 `commands.rs` 经 IPC 暴露的对话编排已迁入 `chat::ChatEngine`，IPC 胶水本身不再需要）。

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// 以下模块自 v1（tag `v1-final`）原样移植，沿用其注释风格；
// v1 未按 `missing_docs` 要求书写，逐项补文档不属于移植范围。
#[allow(missing_docs)]
pub mod chat;
#[allow(missing_docs)]
pub mod mcp;
#[allow(missing_docs)]
pub mod models;
#[allow(missing_docs)]
pub mod providers;
#[allow(missing_docs)]
pub mod storage;
#[allow(missing_docs)]
pub mod streaming;
#[allow(missing_docs)]
pub mod tools;

