//! 更新错误。`Display` 即界面上显示的中文文案。

/// 更新流程中的失败；任何一种都不影响聊天与下次启动。
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("当前平台暂不支持自动更新")]
    UnsupportedPlatform,
    #[error("无法连接更新服务器，请检查网络连接后重试")]
    Network(String),
    #[error("暂时无法获取有效的版本信息，请稍后重试")]
    Manifest(String),
    #[error("更新包下载不完整或已损坏，请重新检查")]
    Integrity(String),
    #[error("更新包签名验证失败，请联系开发者")]
    Signature(String),
    #[error("{0}")]
    Install(String),
}

impl UpdateError {
    /// 记录日志用的技术细节（不显示给用户）。
    pub fn detail(&self) -> &str {
        match self {
            Self::UnsupportedPlatform => "unsupported platform",
            Self::Network(d)
            | Self::Manifest(d)
            | Self::Integrity(d)
            | Self::Signature(d)
            | Self::Install(d) => d,
        }
    }
}
