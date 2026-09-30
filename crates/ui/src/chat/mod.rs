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
//! - [`web_search`] —— 网络搜索卡片（S05-11）
//! - [`message_actions`] —— 回答操作栏（S05-14）
//! - [`empty_page`] / [`no_key_page`] —— 空态页与无 Key 页（S05-16）
//! - [`approval_panel`] —— 工具审批浮层（S05-13）
//! - [`ask_card`] —— ask_user 提问卡（S05-13）
//! - [`model_menu`] —— 模型选择菜单（S05-15）
//! - [`page_state`] / [`chat_page`] / [`router`] —— 页面状态机、对话页、路由器与 engine 接入（S05-18）

pub mod approval_panel;
pub mod ask_card;
pub mod chat_page;
pub mod composer;
pub mod empty_page;
pub mod message_actions;
pub mod message_row;
pub mod model_menu;
pub mod page_state;
pub mod no_key_page;
pub mod router;
pub mod rows;
pub mod session;
pub mod state;
pub mod think_block;
pub mod tool_card;
pub mod transcript;
pub mod web_search;

/// 注册聊天界面所需的键位（输入框）。须在创建窗口前调用一次。
pub fn init(cx: &mut gpui::App) {
    crate::text_area::bind_keys(cx);
}
