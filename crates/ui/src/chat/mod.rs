//! 聊天界面（Phase 05）
//!
//! - [`state`] —— 对话状态：消费 engine 的流式事件（S05-17），纯数据逻辑
//! - [`rows`] —— 块粒度行模型与稳定 id、最小 splice（S05-02），纯函数

pub mod rows;
pub mod state;
