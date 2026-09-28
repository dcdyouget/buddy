//! 聊天界面（Phase 05）
//!
//! - [`state`] —— 对话状态：消费 engine 的流式事件（S05-17），纯数据逻辑
//! - [`rows`] —— 块粒度行模型与稳定 id、最小 splice（S05-02），纯函数
//! - [`session`] —— 会话实体：接收事件批次、按帧推进节奏器、只在变化时通知（S05-17）
//! - [`transcript`] —— 虚拟化消息列表（S05-01）
//! - [`message_row`] —— 消息行外观（S05-08）

pub mod message_row;
pub mod rows;
pub mod session;
pub mod state;
pub mod transcript;
