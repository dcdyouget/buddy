//! 设置页快捷键录制器。
//!
//! 录制器只负责从窗口键盘事件得到 v1 兼容的明文组合键。全局注册、冲突检查和
//! 配置持久化由 Router 负责；录制成功会发出候选值，页面在保存成功后再
//! 用 [`HotkeyRecorder::set_current_hotkey`] 发布到界面。

#[path = "hotkey_state.rs"]
mod state;

use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent,
    KeyUpEvent, ModifiersChangedEvent, Pixels, Render, SharedString, Window, canvas, div,
    prelude::*, px,
};
use std::cell::Cell;
use std::rc::Rc;

use state::{RecordOutcome, RecorderState, display_keys};

/// 用户完成一次录制后发出的候选快捷键。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyChanged(pub String);

/// 快捷键录制器实体。
pub struct HotkeyRecorder {
    current: String,
    state: RecorderState,
    active: bool,
    saving: bool,
    error: Option<SharedString>,
    focus: FocusHandle,
    button_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl EventEmitter<HotkeyChanged> for HotkeyRecorder {}

impl Focusable for HotkeyRecorder {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl HotkeyRecorder {
    /// 使用配置中的明文快捷键创建录制器。
    pub fn new(current: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            current: current.into(),
            state: RecorderState::default(),
            active: true,
            saving: false,
            error: None,
            focus: cx.focus_handle(),
            button_bounds: Rc::new(Cell::new(None)),
        }
    }

    /// 同步外部配置；保存失败时调用 [`save_failed`]，不要提前调用此方法。
    pub fn set_current_hotkey(&mut self, current: impl Into<String>, cx: &mut Context<Self>) {
        let current = current.into();
        if self.current == current {
            return;
        }
        self.current = current;
        self.error = None;
        cx.notify();
    }

    /// 设置覆盖层是否接受输入。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        if !active && self.state.recording {
            self.state.cancel();
        }
        cx.notify();
    }

    /// 保存期间禁用重新录制按钮，但已提交的旧值仍保持可见。
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.saving = saving;
        if saving {
            self.error = None;
        }
        cx.notify();
    }

    /// 保存失败时保留旧快捷键并显示可读提示。
    pub fn save_failed(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.saving = false;
        self.error = Some(message.into());
        cx.notify();
    }

    /// 当前是否处于录制态；父级 Tab/Esc 导航在此状态下应让事件继续交给录制器。
    pub fn recording(&self) -> bool {
        self.state.recording
    }

    /// 当前成功发布的快捷键。
    pub fn current(&self) -> &str {
        &self.current
    }

    /// 是否正在等待写盘完成。
    pub fn saving(&self) -> bool {
        self.saving
    }

    /// SettingsView 的 Tab 导航接口；保存或退出时不加入导航序列。
    pub fn focus_control(&self, _: &App) -> Option<FocusHandle> {
        (self.active && !self.saving).then(|| self.focus.clone())
    }

    /// 设置页滚动定位使用的按钮边界别名。
    pub fn button_bounds(&self) -> Option<Bounds<Pixels>> {
        self.button_bounds.get()
    }

    /// 设置页滚动定位使用的按钮边界测试接口。
    pub fn button_bounds_for_test(&self) -> Option<Bounds<Pixels>> {
        self.button_bounds()
    }

    /// 当前平台格式化后的键帽文字，供预览自测核对 v1 显示。
    pub fn display_keys_for_test(&self) -> Vec<String> {
        display_keys(&self.current, &self.state)
    }

    /// 当前保存错误，供设置页自测核对旧值保留。
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn start_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        self.state.start();
        self.error = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        if !self.state.recording {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                self.start_recording(window, cx);
                cx.stop_propagation();
            }
            return;
        }
        let outcome = self
            .state
            .key_down(event.keystroke.modifiers, &event.keystroke.key);
        cx.stop_propagation();
        if let RecordOutcome::Changed(value) = outcome {
            cx.emit(HotkeyChanged(value));
        }
        cx.notify();
    }

    fn key_up(&mut self, event: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.active || !self.state.recording {
            return;
        }
        let outcome = self
            .state
            .key_up(event.keystroke.modifiers, &event.keystroke.key);
        cx.stop_propagation();
        if let RecordOutcome::Changed(value) = outcome {
            cx.emit(HotkeyChanged(value));
        }
        cx.notify();
    }

    fn modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.active || !self.state.recording {
            return;
        }
        self.state.modifiers_changed(event.modifiers);
        cx.stop_propagation();
        cx.notify();
    }
}

#[path = "hotkey/render.rs"]
mod render;
