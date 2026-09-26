//! S00-07 产物：`ListState` 虚拟列表的正确用法与四个陷阱（macOS / gpui rev 290cbcb）
//!
//! **为什么需要这个文件**：`spikes/` 被 `.gitignore` 排除，spike 代码会丢失。
//! 本文件固化已实测可用的用法，后续 spec 直接取用：
//!   - `S05-01` Transcript 虚拟列表骨架
//!   - `S05-02` 块粒度行模型与稳定 id
//!   - `S05-03` 行高记忆与测量
//!   - `S05-04` 跟尾弹簧与回到底部
//!
//! 实测依据见 `docs/specs/phase-00/S00-07-list.md`。
//!
//! # 实测结论摘要
//!
//! | 项 | 结果 |
//! |----|------|
//! | 虚拟化 | ✅ **3.55 行渲染/帧** vs 总行数 1005（**0.35%**） |
//! | 首帧成本（`measure_all()`，1000 行） | **249 ms** |
//! | 跟尾（`FollowMode::Tail` + 显式 `scroll_to_end()`） | ✅ 状态机全程正确 |
//! | 用户上滑打断 / 回底恢复 | ✅ 自动 |
//!
//! **对比现有 v1**：`src/pages/ChatPage.tsx:402` 是 `visible.map(...)` + 手动分页，
//! 全仓无任何虚拟化，且该文件注释已承认「流式更新会让整页和所有历史消息重复渲染」。

#![allow(dead_code)]

use gpui::{
    AnyElement, App, Context, FollowMode, IntoElement, ListAlignment, ListState, Pixels, Window,
    div, list, prelude::*, px,
};

// ═══════════════════════════════════════════════════════════════
// 陷阱 1：`list()` 必须自己 `flex_grow_1()`，否则**静默渲染 0 行**
// ═══════════════════════════════════════════════════════════════
//
// ## 症状
// 窗口空白、**不报错、不警告**、日志里也看不出异常。
//
// ## 原因
// `List` 虽然实现了 `Styled`（`gpui/src/elements/list.rs:1632`），
// 但其 `ListSizingBehavior::Auto` 分支走的是：
//
// ```ignore
// ListSizingBehavior::Auto => {
//     let mut style = Style::default();
//     style.refine(&self.style);
//     window.request_layout(style, None, cx)      // ← 没有 flex_grow
// }
// ```
//
// 所以 `List` 拿到的是祖先容器里的**自然尺寸 0**。
//
// ## 错误写法（本 spike 踩过）
// ```ignore
// div().flex_1().child(
//     list(state, render)            // ← 包一层 flex_1 容器**不管用**
//         .with_sizing_behavior(ListSizingBehavior::Auto)
// )
// ```
//
// ## 正确写法（与 zed 一致）
// ```ignore
// list(state, render)
//     .with_sizing_behavior(ListSizingBehavior::Auto)
//     .flex_grow_1()                 // ← 必须给 List 自己
//     .w_full()
// ```
//
// 参考：`agent_ui/src/conversation_view/thread_view.rs:6113-6115`
// （zed 的 agent 聊天就是 `.with_sizing_behavior(Auto).flex_grow_1()`）

// ═══════════════════════════════════════════════════════════════
// 陷阱 2：`set_scroll_handler` 回调内**不得调用 `ListState` 任何访问器**
// ═══════════════════════════════════════════════════════════════
//
// ## 症状
// ```
// panicked at gpui/src/elements/list.rs:485:
// RefCell already mutably borrowed
// ```
// 注意：**只有在列表真正开始渲染之后才会崩** —— 若因陷阱 1 而渲染 0 行，
// 不会有滚动事件，也永远不会崩。所以这个 bug 会被陷阱 1 掩盖。
//
// ## 原因
// gpui 在**持有 `&mut *state.0.borrow_mut()` 期间**调用你的回调：
//
// ```ignore
// // list.rs 内部（已持有可变借用）
// if let Some(handler) = self.scroll_handler.as_mut() {
//     handler(&ListScrollEvent { … }, window, cx);   // ← 此刻 state 被可变借用
// }
// ```
//
// 而 `is_scrolled_to_end()` / `is_following_tail()` / `logical_scroll_top()` 等
// 都会做 `self.0.borrow()` → **必然 panic**。
//
// ## 正确做法
// **只读 `ListScrollEvent` 的字段**（它自带所需信息）：
//
// ```ignore
// pub struct ListScrollEvent {
//     pub visible_range: Range<usize>,
//     pub count: usize,
//     pub is_scrolled: bool,
//     pub is_following_tail: bool,
// }
// ```
//
// 若确实需要 `is_scrolled_to_end()`，请在**定时器**或 **`render()`** 里读
// （那时无借用）。本 spike 的做法：回调写 `Rc<RefCell<ScrollInfo>>`，
// 定时器补 `at_end`，`render()` 只读 `ScrollInfo`。
pub fn set_scroll_handler_correctly(state: &ListState, info: std::rc::Rc<std::cell::RefCell<ScrollInfo>>) {
    state.set_scroll_handler(move |event, _window, _cx| {
        // ✅ 只碰 event
        let mut i = info.borrow_mut();
        i.visible = event.visible_range.clone();
        i.count = event.count;
        i.following = event.is_following_tail;
        i.scrolled = event.is_scrolled;
        // ❌ 绝不写：state.is_scrolled_to_end() / state.logical_scroll_top() / …
    });
}

#[derive(Clone, Debug, Default)]
pub struct ScrollInfo {
    pub visible: std::ops::Range<usize>,
    pub count: usize,
    pub following: bool,
    pub scrolled: bool,
    /// 由定时器填充（那时无借用，安全）
    pub at_end: Option<bool>,
}

// ═══════════════════════════════════════════════════════════════
// 陷阱 3：`ListAlignment::Bottom` + 行高未知 = **什么都不渲染**
// ═══════════════════════════════════════════════════════════════
//
// ## 症状
// `is_scrolled_to_end()` 返回 `None`，视口内看不到任何行。
//
// ## 原因
// `ListAlignment::Bottom` **必须知道总高**才能定位「底部」：
//
// ```ignore
// // list.rs:484-490
// pub fn is_scrolled_to_end(&self) -> Option<bool> {
//     let state = self.0.borrow();
//     let bounds = state.last_layout_bounds?;
//     let summary = state.items.summary();
//     if summary.has_unknown_height {
//         return None;                    // ← 全是未知高度 → 无法定位
//     }
//     …
// }
// ```
//
// 而行高来自 `ListItem::size_hint()`，`list()` 的闭包返回 `AnyElement`，
// **无法提供 size hint**，所以新行一律 `has_unknown_height = true`。
//
// ## 实测对比（1000 行变高文本）
//
// | 构造方式 | 首帧渲染行数 | `is_scrolled_to_end()` | 可见 |
// |---------|------------|----------------------|------|
// | `new(1000, Bottom, …)`（无测量） | 6 | `None` | ❌ |
// | 增量 `splice`（无测量） | 126 | `None` | ❌ |
// | **`measure_all()`** | **1000** | **`Some(true)`** | ✅ |
// | `with_uniform_item_height()` | — | — | ✅（但仅限固定行高） |
//
// ## 结论：Buddy 必须用 `measure_all()`
//
// ```ignore
// let state = ListState::new(0, ListAlignment::Bottom, px(1000.))
//     .measure_all();                     // ← 必须
// state.splice(0..0, initial_count);
// state.scroll_to_end();
// ```
//
// ## 代价（实测）
//
// ```
// [measure_all] 1000 行耗时 167 ns        ← 它只设一个标志
// [首帧] 渲染行数 = 1000 / 总行数 1000，耗时 249 ms   ← 真正的 O(n) 成本在首帧
// ```
//
// **~249 ms / 1000 行**，一次性。100 条消息时约 25 ms，可忽略。
//
// > **给 `S05-03` 的提示**：若 249 ms 不可接受，可考虑
// > (a) 首屏只 `splice` 最近 N 条，滚动到顶时再补历史（配合 `S04-04` 的增量加载）
// > (b) 为每种行类型维护高度估计，但 `list()` 未暴露 size hint 接口，
// >     需要改 gpui（属 fork 范畴，见 `S01-02`）

// ═══════════════════════════════════════════════════════════════
// 陷阱 4：`FollowMode::Tail` **只维护状态**，不负责实际滚动
// ═══════════════════════════════════════════════════════════════
//
// ## 症状
// `is_following_tail()` 返回 `true`，但视口**不跟随**新内容。
//
// ## 原因
// `FollowMode::Tail` 只设置 `follow_state = Tail { is_following: true }`，
// 而 `is_following_tail()` 只是**报告这个标志**。它仅在滚轮上滑时被 `stop_following()`：
//
// ```ignore
// // list.rs:940
// if delta.y > px(0.) {
//     self.follow_state.stop_following();
// }
// ```
//
// 真正把视口钉到底部的是 **`scroll_to_end()`**：
//
// ```ignore
// // list.rs:603
// pub fn scroll_to_end(&self) {
//     let state = &mut *self.0.borrow_mut();
//     let item_count = state.items.summary().count;
//     state.pending_scroll = None;
//     state.logical_scroll_top = Some(ListOffset { item_ix: item_count, offset_in_item: px(0.) });
// }
// ```
//
// ## 正确做法：两者都要
//
// ```ignore
// // 初始化
// state.set_follow_mode(FollowMode::Tail);
//
// // 每次追加/内容增高后（且仍处于跟尾状态时）
// if state.is_following_tail() {
//     state.scroll_to_end();
// }
// ```
//
// 参考：`agent_ui/src/conversation_view/thread_view.rs:1692`
// `this.list_state.scroll_to_end();`
//
// ## 状态机实测（用户真实滚动）
// ```text
// [sample 1s] at_end=Some(true)  following=true     ← 贴底
// [sample 3s] at_end=Some(false) following=false    ← 用户上滑 → 自动停跟随
// [sample 5s] at_end=Some(true)  following=true     ← 回底 → 自动恢复
// [sample 7s] at_end=Some(false) following=false
// [sample 11s] at_end=Some(true) following=true
// ```
// 打断与恢复**都是自动的**，无需自己实现 70px 吸附带。

// ═══════════════════════════════════════════════════════════════
// 推荐用法（完整骨架）
// ═══════════════════════════════════════════════════════════════

/// 创建 Buddy 的 transcript 列表。
///
/// - `item_count`：已有行数（历史消息）
/// - `overdraw`：视口外预渲染距离。zed 的 `threads_archive_view` 用 `px(1000.)`；
///   `telemetry_log` 用 `px(2048.)`。**越大越不易在白屏边缘露白，但渲染更多行**。
pub fn create_transcript_list(item_count: usize, overdraw: Pixels) -> ListState {
    // 注意：`measure_all()` 是 Bottom 对齐 + 变高行的**必要**条件（见陷阱 3）
    let state = ListState::new(0, ListAlignment::Bottom, overdraw).measure_all();
    state.splice(0..0, item_count);
    state.set_follow_mode(FollowMode::Tail);
    state.scroll_to_end();
    state
}

/// 渲染列表（注意 `flex_grow_1`，见陷阱 1）
pub fn render_transcript(
    state: ListState,
    render_row: impl FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static,
) -> impl IntoElement {
    list(state, render_row)
        .with_sizing_behavior(gpui::ListSizingBehavior::Auto)
        .flex_grow_1()
        .w_full()
}

/// 追加一行（新消息 / 流式新块）
pub fn append_row(state: &ListState, ix: usize) {
    state.splice(ix..ix, 1);
    if state.is_following_tail() {
        state.scroll_to_end(); // ← 必须显式（见陷阱 4）
    }
}

/// 流式更新某一行的内容后：**只重测这一行**
pub fn remeasure_row_after_stream(state: &ListState, ix: usize) {
    state.remeasure_items(ix..ix + 1); // ← 只影响一行，历史行不重测
    if state.is_following_tail() {
        state.scroll_to_end();
    }
}

// ═══════════════════════════════════════════════════════════════
// 其他有用的 API（本 spike 未全部验证，供 S05 参考）
// ═══════════════════════════════════════════════════════════════
//
// | 需求 | API |
// |------|-----|
// | 是否在底部 | `is_scrolled_to_end() -> Option<bool>` |
// | 是否跟尾 | `is_following_tail() -> bool` |
// | 暂停跟尾 | `pause_following_tail()` |
// | 可见行范围 | `ListScrollEvent.visible_range` |
// | 滚动到指定行 | `scroll_to_reveal_item(ix)` |
// | 某行的屏幕矩形 | `bounds_for_item(ix) -> Option<Bounds<Pixels>>` |
// | 行是否在视口上方 | `item_is_above_viewport(ix) -> Option<bool>` |
// | 行是否在视口下方 | `item_is_below_viewport(ix) -> Option<bool>` |
// | 滚动条 | `max_offset_for_scrollbar()` / `set_offset_from_scrollbar(point)` |
// | 视口范围 | `viewport_bounds() -> Bounds<Pixels>` |
// | 逻辑滚动位置 | `logical_scroll_top() -> ListOffset { item_ix, offset_in_item }` |
// | 重置行数 | `reset(count)` / `reset_with_uniform_height(count, h)` |
// | 标记重测 | `remeasure()` / `remeasure_items(range)` |
//
// **未验证**（`S05-*` 需自行验证）：
// - `splice_focusable` 的焦点管理
// - `item_is_above_viewport` / `item_is_below_viewport` 在流式加高时的行为
// - 视口上方行高变化时的滚动锚点保持（现有 v1 的 `bottomFollow.ts` 需求）
// - 与 markdown 行的协同（本 spike 用纯文本行）
