//! 生产窗口的尺寸与原生创建选项（S07-01）。

use gpui::{
    App, Bounds, Pixels, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, px,
    size,
};

/// 逻辑像素窗口尺寸。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogicalSize {
    /// 窗口宽度。
    pub width: u32,
    /// 窗口高度。
    pub height: u32,
}

impl LogicalSize {
    /// 创建逻辑尺寸。
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// 转换为 GPUI 使用的逻辑像素尺寸。
    pub fn to_gpui(self) -> gpui::Size<Pixels> {
        size(px(self.width as f32), px(self.height as f32))
    }
}

/// 紧凑页的初始尺寸。
pub const COMPACT_SIZE: LogicalSize = LogicalSize::new(560, 60);
/// 对话页的展开尺寸。
pub const CONVERSATION_SIZE: LogicalSize = LogicalSize::new(750, 500);
/// 设置页的展开尺寸。
pub const SETTINGS_SIZE: LogicalSize = LogicalSize::new(760, 640);
/// 原生窗口允许的最小尺寸。
pub const MIN_WINDOW_SIZE: LogicalSize = LogicalSize::new(360, 60);
/// 产品窗口首次创建时的尺寸。
pub const INITIAL_WINDOW_SIZE: LogicalSize = COMPACT_SIZE;

/// 生产窗口创建时使用的配置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellConfig {
    /// 首次创建窗口的尺寸。
    pub initial_size: LogicalSize,
    /// 原生窗口允许的最小尺寸。
    pub min_size: LogicalSize,
    /// 用户是否可以调整窗口尺寸。
    pub resizable: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            initial_size: INITIAL_WINDOW_SIZE,
            min_size: MIN_WINDOW_SIZE,
            resizable: true,
        }
    }
}

impl ShellConfig {
    /// 检查尺寸配置，避免初始窗口小于原生最小窗口。
    pub fn validate(self) -> Result<(), String> {
        if self.min_size.width == 0 || self.min_size.height == 0 {
            return Err("窗口最小尺寸必须大于 0".to_string());
        }
        if self.initial_size.width < self.min_size.width
            || self.initial_size.height < self.min_size.height
        {
            return Err("窗口初始尺寸不能小于最小尺寸".to_string());
        }
        Ok(())
    }

    /// 创建窗口选项并返回配置错误。
    pub fn window_options(&self, cx: &App) -> Result<WindowOptions, String> {
        self.validate()?;
        let bounds = Bounds::centered(None, self.initial_size.to_gpui(), cx);
        Ok(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: false,
            // macOS 先补丁再显示；其他平台沿用 GPUI 不激活的创建路径。
            show: cfg!(not(target_os = "macos")),
            kind: WindowKind::PopUp,
            is_resizable: self.resizable,
            window_min_size: Some(self.min_size.to_gpui()),
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_config_matches_product_geometry() {
        let config = ShellConfig::default();
        assert_eq!(config.initial_size, LogicalSize::new(560, 60));
        assert_eq!(config.min_size, LogicalSize::new(360, 60));
        assert!(config.resizable);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn invalid_minimum_dimensions_are_reported_in_chinese() {
        let config = ShellConfig {
            min_size: LogicalSize::new(0, 60),
            ..ShellConfig::default()
        };
        assert_eq!(config.validate(), Err("窗口最小尺寸必须大于 0".to_string()));

        let config = ShellConfig {
            initial_size: LogicalSize::new(359, 60),
            ..ShellConfig::default()
        };
        assert_eq!(
            config.validate(),
            Err("窗口初始尺寸不能小于最小尺寸".to_string())
        );
    }

    #[test]
    fn logical_size_conversion_preserves_both_axes() {
        let size = LogicalSize::new(750, 500).to_gpui();
        assert_eq!(f32::from(size.width), 750.0);
        assert_eq!(f32::from(size.height), 500.0);
    }
}
