//! 页面切换时的窗口尺寸策略（S07-01）。

use crate::chat::page_state::Page;

use super::config::{COMPACT_SIZE, CONVERSATION_SIZE, LogicalSize, SETTINGS_SIZE};

/// 返回页面的标准展开尺寸。
pub fn page_size(page: Page) -> LogicalSize {
    match page {
        Page::Empty | Page::NoApiKey => COMPACT_SIZE,
        Page::Conversation | Page::Streaming => CONVERSATION_SIZE,
        Page::Settings => SETTINGS_SIZE,
    }
}

/// 返回页面切换需要应用的标准尺寸。
///
/// `None` 表示保留原生窗口当前尺寸，因此用户手动调整后的尺寸不会在普通
/// 内容页切换中被覆盖。`settings_origin` 用于识别从紧凑页进入设置后返回对话。
pub fn resize_target(from: Page, to: Page, settings_origin: Option<Page>) -> Option<LogicalSize> {
    if !from.is_compact() && to.is_compact() {
        return Some(COMPACT_SIZE);
    }

    if from.is_compact() && !to.is_compact() {
        return Some(page_size(to));
    }

    if from == Page::Settings
        && to == Page::Conversation
        && settings_origin.is_some_and(Page::is_compact)
    {
        return Some(CONVERSATION_SIZE);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_sizes_cover_all_routed_pages() {
        assert_eq!(page_size(Page::Empty), COMPACT_SIZE);
        assert_eq!(page_size(Page::NoApiKey), COMPACT_SIZE);
        assert_eq!(page_size(Page::Conversation), CONVERSATION_SIZE);
        assert_eq!(page_size(Page::Streaming), CONVERSATION_SIZE);
        assert_eq!(page_size(Page::Settings), SETTINGS_SIZE);
    }

    #[test]
    fn transition_targets_follow_compact_and_settings_rules() {
        let cases = [
            (
                Page::Empty,
                Page::Conversation,
                None,
                Some(CONVERSATION_SIZE),
            ),
            (Page::NoApiKey, Page::Settings, None, Some(SETTINGS_SIZE)),
            (Page::Conversation, Page::Streaming, None, None),
            (Page::Conversation, Page::Settings, None, None),
            (
                Page::Settings,
                Page::Conversation,
                Some(Page::Empty),
                Some(CONVERSATION_SIZE),
            ),
            (
                Page::Settings,
                Page::Conversation,
                Some(Page::Conversation),
                None,
            ),
            (
                Page::Settings,
                Page::Conversation,
                Some(Page::Streaming),
                None,
            ),
            (Page::Empty, Page::Empty, None, None),
            (Page::NoApiKey, Page::NoApiKey, None, None),
            (Page::Empty, Page::NoApiKey, None, None),
            (Page::Streaming, Page::Settings, None, None),
            (
                Page::NoApiKey,
                Page::Conversation,
                None,
                Some(CONVERSATION_SIZE),
            ),
            (Page::Conversation, Page::Empty, None, Some(COMPACT_SIZE)),
            (Page::Conversation, Page::NoApiKey, None, Some(COMPACT_SIZE)),
        ];

        for (from, to, origin, expected) in cases {
            assert_eq!(
                resize_target(from, to, origin),
                expected,
                "{from:?} -> {to:?}"
            );
        }
    }

    #[test]
    fn content_switch_preserves_an_arbitrary_user_size() {
        let user_size = LogicalSize::new(912, 713);
        let target = resize_target(Page::Conversation, Page::Streaming, None);
        assert_eq!(target.unwrap_or(user_size), user_size);
    }
}
