// 平台特定代码模块

pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;