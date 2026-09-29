//! 页面状态机（S05-18）—— 对应 v1 `uiStore.ts` 的 `currentPage` / `previousPage` 与 `App.tsx` 的路由、配置副作用
//!
//! 纯逻辑，不依赖 GPUI；由 [`super::shell::Shell`] 驱动。
//!
//! ```text
//! startup ── 加载配置 / 历史 ──> empty        （启动后总是 empty：即使已有历史也不自动进入 conversation）
//!
//! empty ── 发送但缺 Provider / 默认模型 ──> noapikey ── 点击 ──> settings
//! empty ── 有效配置发送 ──> streaming ── 完成 / 出错 / 停止 ──> conversation
//! empty ── 点「展开」──> conversation
//! conversation ── 发送 ──> （页面不变；streaming 与 conversation 共用一个对话页）
//! 流式出错且为 401 / unauthorized ──> noapikey
//! empty / conversation / streaming / noapikey ── 设置 ──> settings（叠在上一页之上，不卸载）
//! settings ── 返回 ──> 上一页；上一页是紧凑页（empty / noapikey）时改去 conversation
//! 配置无效且当前不在 settings / empty / noapikey ──> empty
//! 配置补齐且当前在 noapikey ──> conversation
//! ```
//!
//! # 决策（迁自已退役的 `docs/design/pages-and-states.md`）
//!
//! - **设置页不卸载底层页面**：设置只是叠加层，底层的对话 / 流式状态不能被清空，滑入动画才有意义。
//!   → [`PageState::base_page`]。
//! - **添加 Provider 的中间态不能打断设置流程**：依次保存 provider、模型、默认模型的过程中配置暂时无效，
//!   此时若因「配置无效」切回 empty，会提前退回紧凑气泡。→ [`PageState::config_changed`] 对 `settings` 不动作。
//! - **窗口尺寸**：v1 仅在「离开紧凑页（empty / noapikey）进入内容页」时展开一次，内容页之间切换保持用户尺寸
//!   （AGENTS.md 硬约束 6 的准确含义，见 `docs/evidence/v1-baseline/README.md` §3）。
//!   本模块只给出判定 [`expands_window`]；真正改窗口尺寸属 Phase 07，**页面切换本身不改窗口**。

/// 页面（v1 `PageState`；`add-provider` 是 v1 遗留的无路由类型值，不迁移）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// 空态（紧凑输入气泡）
    Empty,
    /// 未配置 API Key
    NoApiKey,
    /// 对话（非流式）
    Conversation,
    /// 对话（流式中）；与 [`Page::Conversation`] 渲染同一页
    Streaming,
    /// 设置（叠加层）
    Settings,
}

impl Page {
    /// 紧凑页（v1 `COMPACT_PAGES`）
    pub fn is_compact(self) -> bool {
        matches!(self, Page::Empty | Page::NoApiKey)
    }
}

/// v1 `resizeWindowForPage`：仅「离开紧凑页、进入非紧凑页」时展开窗口
pub fn expands_window(from: Page, to: Page) -> bool {
    from.is_compact() && !to.is_compact()
}

/// 从空态发送时的处理（v1 `EmptyPage.handleSend`）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptySend {
    /// 空白输入：忽略
    Ignore,
    /// 未配置 Provider 或默认模型：去 noapikey（草稿保留，不发送）
    NeedsKey,
    /// 图片但模型不支持
    ImagesUnsupported,
    /// 发送并进入 streaming
    Send,
}

/// v1 `EmptyPage.handleSend` 的判定顺序：空白 → 缺配置 → 图片不被支持 → 发送
pub fn classify_empty_send(has_content: bool, has_valid_config: bool, has_images: bool, supports_vision: bool) -> EmptySend {
    if !has_content {
        EmptySend::Ignore
    } else if !has_valid_config {
        EmptySend::NeedsKey
    } else if has_images && !supports_vision {
        EmptySend::ImagesUnsupported
    } else {
        EmptySend::Send
    }
}

/// 页面状态
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageState {
    current: Page,
    previous: Option<Page>,
}

impl Default for PageState {
    fn default() -> Self {
        Self::new()
    }
}

impl PageState {
    /// 启动：空态
    pub fn new() -> Self {
        Self { current: Page::Empty, previous: None }
    }

    /// 当前页
    pub fn current(&self) -> Page {
        self.current
    }

    /// 上一页
    pub fn previous(&self) -> Option<Page> {
        self.previous
    }

    /// 实际渲染的底层页面：设置页叠在上一页之上（上一页缺失时回落空态）；其余即当前页
    pub fn base_page(&self) -> Page {
        match self.current {
            Page::Settings => self.previous.unwrap_or(Page::Empty),
            page => page,
        }
    }

    /// v1 `setPage`：相同页忽略；记录上一页。返回是否发生了切换
    pub fn set_page(&mut self, page: Page) -> bool {
        if self.current == page {
            return false;
        }
        self.previous = Some(self.current);
        self.current = page;
        true
    }

    /// v1 `goBack`：回到上一页（没有则忽略）
    pub fn go_back(&mut self) -> bool {
        match self.previous.take() {
            Some(previous) => {
                self.current = previous;
                true
            }
            None => false,
        }
    }

    /// v1 `App.tsx` 设置页 `onBack`：上一页是紧凑页时进入 conversation，否则回到上一页
    pub fn close_settings(&mut self) -> bool {
        let previous = self.previous.unwrap_or(Page::Empty);
        self.set_page(if previous.is_compact() { Page::Conversation } else { previous })
    }

    /// v1 `App.tsx` 的配置副作用（Provider 数量 / 默认模型变化时执行）
    pub fn config_changed(&mut self, has_valid_config: bool) -> bool {
        match (has_valid_config, self.current) {
            // 中间态：设置页内分步保存时不得打断
            (false, Page::Settings | Page::Empty | Page::NoApiKey) => false,
            (false, _) => self.set_page(Page::Empty),
            // 在「无 Key」页被外部补齐配置：直接进入展开的对话页
            (true, Page::NoApiKey) => self.set_page(Page::Conversation),
            (true, _) => false,
        }
    }

    /// 流式结束后的落点（v1 `useStreaming.ts`：done / aborted / 出错后 `setPage`）：
    /// 401 / unauthorized 去 noapikey，其余去 conversation
    pub fn stream_finished(&mut self, needs_api_key: bool) -> bool {
        self.set_page(if needs_api_key { Page::NoApiKey } else { Page::Conversation })
    }
}

/// 配置是否可用于发送（v1：至少一个 Provider 且已选默认模型）
pub fn has_valid_config(provider_count: usize, selected_model_id: &str) -> bool {
    provider_count > 0 && !selected_model_id.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(page: Page) -> PageState {
        let mut s = PageState::new();
        s.set_page(page);
        s
    }

    #[test]
    fn starts_empty_and_ignores_same_page() {
        let mut s = PageState::new();
        assert_eq!((s.current(), s.previous()), (Page::Empty, None));
        assert!(!s.set_page(Page::Empty));
        assert_eq!(s.previous(), None, "相同页不记录上一页");
    }

    #[test]
    fn set_page_records_previous_and_go_back_restores() {
        let mut s = PageState::new();
        assert!(s.set_page(Page::Conversation));
        assert_eq!((s.current(), s.previous()), (Page::Conversation, Some(Page::Empty)));
        assert!(s.go_back());
        assert_eq!((s.current(), s.previous()), (Page::Empty, None));
        assert!(!s.go_back(), "没有上一页时忽略");
    }

    #[test]
    fn settings_overlays_previous_page_and_falls_back_to_empty() {
        let mut s = at(Page::Streaming);
        s.set_page(Page::Settings);
        assert_eq!(s.base_page(), Page::Streaming, "设置页叠在流式页上，不卸载");
        assert_eq!(at(Page::Streaming).base_page(), Page::Streaming);
        let mut orphan = PageState { current: Page::Settings, previous: None };
        assert_eq!(orphan.base_page(), Page::Empty);
        assert!(orphan.close_settings());
        assert_eq!(orphan.current(), Page::Conversation, "上一页缺失按空态处理 → 紧凑页 → conversation");
    }

    #[test]
    fn closing_settings_from_compact_pages_goes_to_conversation() {
        for compact in [Page::Empty, Page::NoApiKey] {
            let mut s = PageState::new();
            s.set_page(compact);
            s.set_page(Page::Settings);
            s.close_settings();
            assert_eq!(s.current(), Page::Conversation, "{compact:?} → settings → conversation");
        }
        for content in [Page::Conversation, Page::Streaming] {
            let mut s = at(content);
            s.set_page(Page::Settings);
            s.close_settings();
            assert_eq!(s.current(), content, "内容页原样返回");
        }
    }

    #[test]
    fn config_changed_follows_v1_effect() {
        // 无效配置：内容页退回 empty；settings / empty / noapikey 不动
        for (page, expect) in [
            (Page::Conversation, Page::Empty),
            (Page::Streaming, Page::Empty),
            (Page::Settings, Page::Settings),
            (Page::Empty, Page::Empty),
            (Page::NoApiKey, Page::NoApiKey),
        ] {
            let mut s = at(page);
            s.config_changed(false);
            assert_eq!(s.current(), expect, "无效配置时 {page:?}");
        }
        // 有效配置：只有 noapikey → conversation
        for (page, expect) in [
            (Page::NoApiKey, Page::Conversation),
            (Page::Empty, Page::Empty),
            (Page::Conversation, Page::Conversation),
            (Page::Settings, Page::Settings),
        ] {
            let mut s = at(page);
            s.config_changed(true);
            assert_eq!(s.current(), expect, "有效配置时 {page:?}");
        }
    }

    #[test]
    fn stream_finished_goes_to_noapikey_only_for_auth_failure() {
        let mut s = at(Page::Streaming);
        assert!(s.stream_finished(false));
        assert_eq!(s.current(), Page::Conversation);
        let mut s = at(Page::Streaming);
        s.stream_finished(true);
        assert_eq!(s.current(), Page::NoApiKey);
    }

    #[test]
    fn empty_send_checks_in_v1_order() {
        use EmptySend::*;
        assert_eq!(classify_empty_send(false, false, true, false), Ignore, "空白优先于缺配置");
        assert_eq!(classify_empty_send(true, false, true, false), NeedsKey, "缺配置优先于图片校验");
        assert_eq!(classify_empty_send(true, true, true, false), ImagesUnsupported);
        assert_eq!(classify_empty_send(true, true, true, true), Send);
        assert_eq!(classify_empty_send(true, true, false, false), Send);
    }

    #[test]
    fn window_expands_only_when_leaving_compact_pages() {
        use Page::*;
        assert!(expands_window(Empty, Conversation));
        assert!(expands_window(NoApiKey, Settings));
        assert!(!expands_window(Empty, NoApiKey), "紧凑页之间不变");
        assert!(!expands_window(Conversation, Settings), "内容页之间保持用户尺寸");
        assert!(!expands_window(Conversation, Empty), "回到紧凑页不由前端缩窗");
    }

    #[test]
    fn valid_config_needs_provider_and_default_model() {
        assert!(has_valid_config(1, "p::m"));
        assert!(!has_valid_config(0, "p::m"));
        assert!(!has_valid_config(1, ""));
    }
}
