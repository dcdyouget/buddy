//! 新增 Provider 复用配置串行队列：先写盘，成功后才发布到 Router 并返回对话。
use super::*;
use crate::settings::model_config::{ModelEdit, apply_model_edit};
use crate::settings::{provider_form::ProviderSubmission, provider_merge::merge_provider};

impl PageRouter {
    pub(super) fn save_provider(&mut self, submission: ProviderSubmission, cx: &mut Context<Self>) {
        let panel = self.settings.read(cx).provider_panel().clone();
        panel.update(cx, |panel, cx| panel.set_saving(true, cx));
        let previous = self.config_save.take();
        let state = self.config_save_state.clone();
        let sequence = state.borrow_mut().begin(None);
        let engine = self.engine.clone();
        let (completed, completion) = tokio::sync::oneshot::channel();
        // 独立前台任务保留队列和快照；窗口实体销毁也不能把已授权的配置保存截断。
        // Router 持有的只是完成信号，后续模型选择仍按同一队列等待。
        cx.spawn(async move |router, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let config = state.borrow().config();
            let candidate = merge_provider(&config, &submission);
            let to_save = candidate.clone();
            let work =
                cx.update(|cx| spawn_engine(cx, async move { engine.save_config(to_save).await }));
            match work.await {
                Ok(()) => {
                    let candidate = state.borrow_mut().complete(sequence, candidate);
                    let _ = router.update(cx, |router, cx| {
                        // 不在保存之前 apply_config，避免不完整配置提前切走设置页。
                        router.publish_config(candidate, cx);
                        router.transition(cx, |pages| pages.set_page(Page::Conversation));
                    });
                }
                Err(error) => {
                    let _ = panel.update(cx, |panel, cx| panel.save_failed(error, cx));
                }
            }
            let _ = completed.send(());
        })
        .detach();
        self.config_save = Some(cx.background_spawn(async move {
            let _ = completion.await;
        }));
    }

    pub(super) fn save_model_edit(&mut self, edit: ModelEdit, cx: &mut Context<Self>) {
        let settings = self.settings.clone();
        settings.update(cx, |view, cx| view.set_model_saving(true, cx));
        let previous = self.config_save.take();
        let state = self.config_save_state.clone();
        let sequence = state.borrow_mut().begin(None);
        let engine = self.engine.clone();
        let (completed, completion) = tokio::sync::oneshot::channel();
        cx.spawn(async move |router, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let candidate = apply_model_edit(&state.borrow().config(), &edit);
            let to_save = candidate.clone();
            let work =
                cx.update(|cx| spawn_engine(cx, async move { engine.save_config(to_save).await }));
            match work.await {
                Ok(()) => {
                    let visible = state.borrow_mut().complete(sequence, candidate);
                    let _ = router.update(cx, |router, cx| router.publish_config(visible, cx));
                    let _ = settings.update(cx, |view, cx| view.set_model_saving(false, cx));
                }
                Err(error) => {
                    let _ = settings.update(cx, |view, cx| view.model_save_failed(error, cx));
                }
            }
            let _ = completed.send(());
        })
        .detach();
        self.config_save = Some(cx.background_spawn(async move {
            let _ = completion.await;
        }));
    }

    pub(super) fn save_model_selection(&mut self, id: String, cx: &mut Context<Self>) {
        let previous = self.config_save.take();
        let state = self.config_save_state.clone();
        let sequence = state.borrow_mut().begin(Some(id.clone()));
        let engine = self.engine.clone();
        let (completed, completion) = tokio::sync::oneshot::channel();
        cx.spawn(async move |router, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let candidate = apply_model_edit(&state.borrow().config(), &ModelEdit::SetDefault(id));
            let to_save = candidate.clone();
            let work =
                cx.update(|cx| spawn_engine(cx, async move { engine.save_config(to_save).await }));
            match work.await {
                Ok(()) => {
                    let visible = state.borrow_mut().complete(sequence, candidate);
                    let _ = router.update(cx, |router, cx| router.publish_config(visible, cx));
                }
                Err(message) => {
                    let _ = router.update(cx, |router, cx| {
                        router.conversation.update(cx, |c, cx| {
                            c.state.error = Some(format!("保存默认模型失败：{message}"));
                            c.state.revision += 1;
                            cx.notify();
                        });
                    });
                }
            }
            let _ = completed.send(());
        })
        .detach();
        self.config_save = Some(cx.background_spawn(async move {
            let _ = completion.await;
        }));
    }
}
