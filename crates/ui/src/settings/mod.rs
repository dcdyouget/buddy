//! 设置页（S06-*）。作为覆盖层保留底层对话与共享 Composer，窗口尺寸由应用外壳管理。

pub mod controls;
pub mod panel;
pub mod select;
pub mod view;

pub use view::{SettingsEvent, SettingsView};
