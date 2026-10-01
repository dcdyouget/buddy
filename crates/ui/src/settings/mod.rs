//! 设置页（S06-*）。作为覆盖层保留底层对话与共享 Composer，窗口尺寸由应用外壳管理。

pub mod controls;
pub mod model_config;
pub mod model_list;
pub mod panel;
pub mod provider;
pub mod provider_form;
pub mod provider_merge;
pub mod provider_presets;
pub mod select;
pub mod view;

pub use view::{SettingsEvent, SettingsView};

#[cfg(test)]
mod model_config_tests;
#[cfg(test)]
mod model_mcp_tests;
