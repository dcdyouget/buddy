//! 自启复用配置串行队列；OS 读回成功后才写盘，写盘失败恢复旧登录项。
use super::*;
use crate::shell::autostart::AutostartBackend;

impl PageRouter {
    /// 事务在后台继续直到结束；丢弃返回任务不会中断 OS 回滚或配置队列。
    pub fn toggle_autostart(
        &mut self,
        backend: Rc<dyn AutostartBackend>,
        cx: &mut Context<Self>,
    ) -> Task<Result<bool, String>> {
        let previous = self.config_save.take();
        let state = self.config_save_state.clone();
        let sequence = state.borrow_mut().begin(None);
        let engine = self.engine.clone();
        let (finished, barrier) = tokio::sync::oneshot::channel();
        let (reply, response) = tokio::sync::oneshot::channel();
        cx.spawn(async move |router, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = async {
                // 在前一项落盘后取基底，避免覆盖同队列里的模型 / 主题 / 热键。
                let mut candidate = state.borrow().config();
                let before = backend.query()?;
                let target = !before;
                backend.set(target)?;
                candidate.auto_start = target;
                let to_save = candidate.clone();
                let work = cx.update(|cx| {
                    spawn_engine(cx, async move { engine.save_config(to_save).await })
                });
                if let Err(error) = work.await {
                    return Err(match backend.set(before) {
                        Ok(()) => format!("保存开机自启失败，已恢复原登录项：{error}"),
                        Err(rollback) => {
                            format!("保存开机自启失败：{error}；原登录项恢复失败：{rollback}")
                        }
                    });
                }
                let visible = state.borrow_mut().complete(sequence, candidate);
                let _ = router.update(cx, |router, cx| router.publish_config(visible, cx));
                Ok(target)
            }
            .await;
            let _ = reply.send(result);
            let _ = finished.send(());
        })
        .detach();
        self.config_save = Some(cx.background_spawn(async move {
            let _ = barrier.await;
        }));
        cx.background_spawn(async move {
            response
                .await
                .unwrap_or_else(|_| Err("开机自启事务未完成".into()))
        })
    }
}
