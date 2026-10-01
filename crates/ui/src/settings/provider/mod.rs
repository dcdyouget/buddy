//! S06-02 添加模型服务面板。

mod focus;
mod panel;
mod panel_init;
mod panel_ops;
mod render;

pub use crate::settings::provider_form::{Busy, ProviderForm, ProviderRequest, ProviderSubmission};
pub use panel::{AddProviderPanel, ProviderEvent};
