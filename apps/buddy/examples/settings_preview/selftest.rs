use super::input::*;
use super::*;

async fn find_settings_button(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    let composer = handle
        .read_with(cx, |router, _| router.composer().clone())
        .expect("读取 Composer");
    let model = composer.read_with(cx, |composer, _| composer.model_button_bounds());
    let Some(model) = model else { return false };
    let center_y = f32::from(model.origin.y + model.size.height / 2.0);
    // Composer 中设置按钮尺寸 24px、与模型按钮间距 4px；取按钮中心：模型左缘 - 16px。
    let x = f32::from(model.origin.x) - 16.0;
    click(handle, x, center_y, cx).await;
    handle
        .read_with(cx, |router, _| router.page() == Page::Settings)
        .unwrap_or(false)
}

/// 等待真实的流式 pump / Pacer 放出目标文本；每轮都让后台任务运行并重绘窗口。
async fn wait_for_stream_text(
    handle: WindowHandle<PageRouter>,
    conversation: &Entity<buddy_ui::chat::session::Conversation>,
    expected: &str,
    cx: &mut AsyncApp,
) -> String {
    const POLL_INTERVAL: Duration = Duration::from_millis(20);
    const MAX_POLLS: usize = 100; // 最多 100 轮，不手动 tick / flush

    for _ in 0..MAX_POLLS {
        cx.background_executor().timer(POLL_INTERVAL).await;
        draw(handle, cx).await;
        let text = conversation.read_with(cx, |conversation, _| {
            conversation
                .state
                .live
                .as_ref()
                .map(|live| text_of(&live.blocks))
                .unwrap_or_default()
        });
        if text == expected {
            return text;
        }
    }

    conversation.read_with(cx, |conversation, _| {
        conversation
            .state
            .live
            .as_ref()
            .map(|live| text_of(&live.blocks))
            .unwrap_or_default()
    })
}

pub(crate) async fn selftest(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    draw(handle, cx).await;

    let composer = handle
        .read_with(cx, |router, _| router.composer().clone())
        .unwrap();
    let conversation = handle
        .read_with(cx, |router, _| router.conversation().clone())
        .unwrap();
    let draft = "设置页打开期间保留的草稿";
    let _ = cx.update_window(handle.into(), |_, _, cx| {
        composer.update(cx, |composer, cx| composer.set_draft(draft, cx));
    });
    draw(handle, cx).await;
    let before_size = viewport_size(handle, cx);
    let before_size_exact = before_size == Some(size(px(WIDTH), px(HEIGHT)));
    let before_messages =
        conversation.read_with(cx, |conversation, _| conversation.state.messages.len());
    let transcript = handle
        .read_with(cx, |router, app| router.transcript(app))
        .unwrap();
    click(handle, WIDTH / 2.0, 12.0, cx).await;
    draw(handle, cx).await;
    let expanded_to_conversation = handle
        .read_with(cx, |router, _| router.page() == Page::Conversation)
        .unwrap_or(false);
    let transcript_scrollable = transcript.read_with(cx, |transcript, _| {
        transcript.list_state().max_offset_for_scrollbar().y > px(0.0)
    });
    let transcript_scroll_before = transcript.read_with(cx, |transcript, _| {
        let offset = transcript.list_state().logical_scroll_top();
        (offset.item_ix, offset.offset_in_item)
    });

    let entered_by_mouse = find_settings_button(handle, cx).await;
    let active = handle
        .read_with(cx, |router, app| router.settings_view().read(app).active())
        .unwrap_or(false);
    let initial_present = handle
        .read_with(cx, |router, _| router.settings_present())
        .unwrap_or(false);
    let entry_amount = handle
        .read_with(cx, |router, _| router.settings_amount())
        .unwrap_or(0.0);
    let entry_bounds = handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).painted_bounds()
        })
        .unwrap_or(None);
    cx.background_executor()
        .timer(Duration::from_millis(240))
        .await;
    draw(handle, cx).await;
    let settled_present = handle
        .read_with(cx, |router, _| router.settings_present())
        .unwrap_or(false);
    let settled_amount = handle
        .read_with(cx, |router, _| router.settings_amount())
        .unwrap_or(0.0);
    let settled_bounds = handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).painted_bounds()
        })
        .unwrap_or(None);

    let before_scroll = handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).scroll_offset()
        })
        .unwrap_or_else(|_| point(px(0.0), px(0.0)));
    click(handle, WIDTH / 2.0, HEIGHT / 2.0, cx).await;
    wheel(handle, -420.0, cx).await;
    let after_scroll = handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).scroll_offset()
        })
        .unwrap_or_else(|_| point(px(0.0), px(0.0)));
    let scrolled = before_scroll != after_scroll;
    let transcript_scroll_after = transcript.read_with(cx, |transcript, _| {
        let offset = transcript.list_state().logical_scroll_top();
        (offset.item_ix, offset.offset_in_item)
    });
    let transcript_kept = transcript_scroll_before == transcript_scroll_after;
    // 活跃覆盖层必须拦截底层设置入口，页面状态保持在 Settings。
    let covered_bottom_click = find_settings_button(handle, cx).await;
    let covered_page = handle
        .read_with(cx, |router, _| router.page() == Page::Settings)
        .unwrap_or(false);

    // Tab / Shift+Tab 按视觉顺序循环到返回按钮，Enter 通过真实 key handler 返回。
    press(handle, "tab", cx).await;
    press(handle, "shift-tab", cx).await;
    press(handle, "enter", cx).await;
    let closed_by_keyboard = handle
        .read_with(cx, |router, _| router.page() != Page::Settings)
        .unwrap_or(false);
    let inactive_immediately = handle
        .read_with(cx, |router, app| !router.settings_view().read(app).active())
        .unwrap_or(false);
    let still_present_during_exit = handle
        .read_with(cx, |router, _| router.settings_present())
        .unwrap_or(false);

    // 退出动画尚未结束时，底层设置入口应立即可点；再次点开也证明覆盖层已释放输入。
    let reopened_during_exit = find_settings_button(handle, cx).await;
    let _ = cx.update_window(handle.into(), |_, _, cx| {
        conversation.update(cx, |conversation, cx| {
            conversation.begin_send(
                fixture::message("live-user", MessageRole::User, "继续生成"),
                "p1::settings-0",
                cx,
            );
            conversation.apply_events(
                vec![
                    StreamEvent::TextStart { content_index: 0 },
                    StreamEvent::TextDelta {
                        content_index: 0,
                        delta: "第一段".into(),
                    },
                ],
                cx,
            );
        });
    });
    let stream_text_one = wait_for_stream_text(handle, &conversation, "第一段", cx).await;
    let _ = cx.update_window(handle.into(), |_, _, cx| {
        conversation.update(cx, |conversation, cx| {
            conversation.apply_events(
                vec![StreamEvent::TextDelta {
                    content_index: 0,
                    delta: "第二段".into(),
                }],
                cx,
            );
        });
    });
    let stream_text_two = wait_for_stream_text(handle, &conversation, "第一段第二段", cx).await;
    let streaming_in_settings =
        conversation.read_with(cx, |conversation, _| conversation.state.is_streaming());
    let stream_grew = stream_text_one == "第一段"
        && stream_text_two.starts_with("第一段")
        && stream_text_two.len() > stream_text_one.len();
    let stream_complete = stream_text_two == "第一段第二段";
    println!(
        "T35 诊断：stream_text_one={stream_text_one:?}，stream_text_two={stream_text_two:?}，完整第二段 {stream_complete}",
    );
    let messages_in_settings =
        conversation.read_with(cx, |conversation, _| conversation.state.messages.len());

    // 真实鼠标点击设置页返回按钮。
    cx.background_executor()
        .timer(Duration::from_millis(240))
        .await;
    draw(handle, cx).await;
    click(handle, 24.0, 28.0, cx).await;
    let closed_by_mouse = handle
        .read_with(cx, |router, _| router.page() != Page::Settings)
        .unwrap_or(false);
    let draft_kept = composer.read_with(cx, |composer, app| composer.draft(app) == draft);
    let stream_kept =
        conversation.read_with(cx, |conversation, _| conversation.state.is_streaming());
    let messages_kept = conversation.read_with(cx, |conversation, _| {
        conversation.state.messages.len() == messages_in_settings
    });
    let after_size = viewport_size(handle, cx);
    let size_kept = before_size == after_size;
    // SettingsView 自身有 1px 内边框，GPUI 的 size_full 记录内容盒为视口各边扣 2px。
    let expected_content_size =
        before_size.map(|viewport| size(viewport.width - px(2.0), viewport.height - px(2.0)));
    let bounds_size_kept = entry_bounds
        .is_some_and(|bounds| Some(bounds.size) == expected_content_size)
        && settled_bounds.is_some_and(|bounds| Some(bounds.size) == expected_content_size);
    let animation_measured = entry_amount > 0.0
        && settled_amount > 0.99
        && entry_bounds
            .zip(settled_bounds)
            .is_some_and(|(entry, settled)| entry.origin.x > settled.origin.x + px(20.0))
        && settled_bounds.is_some_and(|bounds| bounds.origin.x <= px(1.0));
    let session_kept = before_messages + 2 == messages_in_settings && messages_kept;

    println!(
        "T35: Conversation {expanded_to_conversation}（底层列表可滚动 {transcript_scrollable}）；鼠标进入 {entered_by_mouse}；active {active}；动画 {initial_present}→{settled_present}（amount {entry_amount:.2}→{settled_amount:.2}，bounds {animation_measured}）；\
         覆盖层拦截 {covered_bottom_click} / 页面保持 {covered_page}；滚动 {before_scroll:?}→{after_scroll:?}（变化 {scrolled}，底层列表保持 {transcript_kept}）；\
         Tab/Enter 返回 {closed_by_keyboard}；退出立即释放 {inactive_immediately}；退出中仍绘制 {still_present_during_exit}；底层立即可点 {reopened_during_exit}；\
         设置中流式 {streaming_in_settings}（文本继续 {stream_grew}）；鼠标返回 {closed_by_mouse}；草稿保留 {draft_kept}；流式保留 {stream_kept}；会话保留 {session_kept}；尺寸 {before_size_exact}→{size_kept}（内容尺寸 {bounds_size_kept}，{entry_bounds:?}→{settled_bounds:?}）"
    );
    let ok = expanded_to_conversation
        && transcript_scrollable
        && entered_by_mouse
        && active
        && initial_present
        && settled_present
        && animation_measured
        && covered_bottom_click
        && covered_page
        && scrolled
        && transcript_kept
        && closed_by_keyboard
        && inactive_immediately
        && still_present_during_exit
        && reopened_during_exit
        && streaming_in_settings
        && stream_grew
        && stream_complete
        && closed_by_mouse
        && draft_kept
        && stream_kept
        && session_kept
        && size_kept;
    println!(
        "{} S06-01 T35 设置层 Router 交互 / 尺寸 / 状态保留",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}
