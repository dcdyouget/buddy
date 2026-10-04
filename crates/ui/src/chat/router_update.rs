//! 自更新安装完成后的重启时机（S08-05）：流式回复中不重启，等回复结束。

use super::PageRouter;
use gpui::Context;

impl PageRouter {
    /// 新版本已替换到磁盘：空闲时立即重启，流式中推迟到回复结束。
    pub(super) fn restart_for_update(
        &mut self,
        installed: buddy_update::Installed,
        cx: &mut Context<Self>,
    ) {
        if self.conversation.read(cx).state.is_streaming() {
            self.pending_update = Some(installed);
            self.settings.update(cx, |settings, cx| {
                settings
                    .update_control()
                    .update(cx, |control, cx| control.wait_for_idle(cx))
            });
            return;
        }
        // 先派生「等本进程退出再打开新应用」的进程，再走与托盘「退出」相同的路径
        if let Err(error) = buddy_update::relaunch_after_exit(&installed) {
            self.settings.update(cx, |settings, cx| {
                settings
                    .update_control()
                    .update(cx, |control, cx| control.restart_failed(&error, cx))
            });
            return;
        }
        let save = self.take_config_save();
        cx.spawn(async move |_, cx| {
            if let Some(save) = save {
                save.await;
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}
