//! 聊天界面（Phase 05）
//!
//! - [`state`] —— 对话状态：消费 engine 的流式事件（S05-17），纯数据逻辑
//! - [`rows`] —— 块粒度行模型与稳定 id、最小 splice（S05-02），纯函数
//! - [`session`] —— 会话实体：接收事件批次、按帧推进节奏器、只在变化时通知（S05-17）
//! - [`transcript`] —— 虚拟化消息列表（S05-01）
//! - [`message_row`] —— 消息行外观（S05-08）
//! - [`composer`] —— 输入区（S05-06）
//! - [`think_block`] —— 思考块（S05-09）
//! - [`tool_card`] —— 工具调用卡片（S05-10）
//! - [`message_actions`] —— 回答操作栏（S05-14）
//! - [`empty_page`] / [`no_key_page`] —— 空态页与无 Key 页（S05-16）

pub mod composer;
pub mod empty_page;
pub mod message_actions;
pub mod message_row;
pub mod no_key_page;
pub mod rows;
pub mod session;
pub mod state;
pub mod think_block;
pub mod tool_card;
pub mod transcript;

/// 注册聊天界面所需的键位（输入框）。须在创建窗口前调用一次。
pub fn init(cx: &mut gpui::App) {
    crate::text_area::bind_keys(cx);
}
