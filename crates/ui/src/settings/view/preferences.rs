use super::*;

impl SettingsView {

    /// 外观控件（真实点击与配置恢复核查）。
    pub fn theme_control(&self) -> &gpui::Entity<super::super::theme_control::ThemeControl> {
        &self.theme
    }

    /// 软件更新区域（Router 设置重启状态、自检读取）。
    pub fn update_control(&self) -> &gpui::Entity<super::super::update::UpdateControl> {
        &self.update
    }

    pub(super) fn sync_preferences(&self, cx: &mut Context<Self>) {
        self.hotkey.update(cx, |control, cx| {
            control.set_current_hotkey(self.config.hotkey.clone(), cx)
        });
        self.theme.update(cx, |control, cx| {
            control.set_theme(self.config.theme.clone(), cx)
        });
        self.font_size.update(cx, |control, cx| {
            control.set_size(self.config.font_size, cx)
        });
    }

    pub(super) fn sync_preference_active(&self, cx: &mut Context<Self>) {
        let active = self.active && !self.provider_motion.interactive();
        self.hotkey
            .update(cx, |control, cx| control.set_active(active, cx));
        self.theme
            .update(cx, |control, cx| control.set_active(active, cx));
        self.font_size
            .update(cx, |control, cx| control.set_active(active, cx));
        self.update
            .update(cx, |control, cx| control.set_active(active, cx));
    }

    pub(crate) fn set_preference_saving(&self, kind: PreferenceKind, saving: bool, cx: &mut Context<Self>) {
        match kind {
            PreferenceKind::Hotkey => self
                .hotkey
                .update(cx, |control, cx| control.set_saving(saving, cx)),
            PreferenceKind::Theme => self
                .theme
                .update(cx, |control, cx| control.set_saving(saving, cx)),
            PreferenceKind::FontSize => self
                .font_size
                .update(cx, |control, cx| control.set_saving(saving, cx)),
        }
    }

    pub(crate) fn preference_failed(&self, kind: PreferenceKind, error: String, cx: &mut Context<Self>) {
        match kind {
            PreferenceKind::Hotkey => self.hotkey.update(cx, |control, cx| {
                control.save_failed(format!("保存快捷键失败：{error}"), cx)
            }),
            PreferenceKind::Theme => self
                .theme
                .update(cx, |control, cx| control.save_failed(error, cx)),
            PreferenceKind::FontSize => self
                .font_size
                .update(cx, |control, cx| control.save_failed(error, cx)),
        }
    }
}
