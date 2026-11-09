use super::*;
use buddy_engine::storage::HistoryMatch;

impl PageRouter {
    /// 切换历史搜索；紧凑气泡及设置页不处理此命令。
    pub fn toggle_history_search(&mut self, cx: &mut Context<Self>) {
        self.transition(cx, |pages| pages.toggle_history_search());
    }

    pub(super) fn close_history_search(&mut self, cx: &mut Context<Self>) {
        if self.page() == Page::HistorySearch {
            self.toggle_history_search(cx);
        }
    }

    pub(super) fn locate_history(&mut self, result: HistoryMatch, cx: &mut Context<Self>) {
        if self.page() != Page::HistorySearch {
            return;
        }
        self.location_generation += 1;
        let generation = self.location_generation;
        self.history_search
            .update(cx, |search, cx| search.set_locating(true, None, cx));
        let read = self.conversation.update(cx, |conversation, cx| {
            conversation.load_through(result.offset(), cx)
        });
        self.location_task = Some(cx.spawn(async move |this, cx| {
            let loaded = read.await;
            let _ = this.update(cx, |this, cx| {
                this.location_task = None;
                this.history_search
                    .update(cx, |search, cx| search.set_locating(false, None, cx));
                if generation != this.location_generation || this.page() != Page::HistorySearch {
                    return;
                }
                let error = match loaded {
                    Err(error) => Some(error),
                    Ok(()) => {
                        let located = this.transcript(cx).update(cx, |transcript, cx| {
                            transcript.reveal_search_result(result.message_id(), cx)
                        });
                        if located {
                            this.close_history_search(cx);
                            None
                        } else {
                            Some("没有找到这条历史消息，请重新查询".into())
                        }
                    }
                };
                if error.is_some() {
                    this.history_search
                        .update(cx, |search, cx| search.set_locating(false, error, cx));
                }
            });
        }));
    }
}
