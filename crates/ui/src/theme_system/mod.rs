//! Buddy 主题体系（S03-*）
//!
//! 令牌数据由 `scripts/theme/gen_tokens.py` 从 v1 代码真值（`v1-final:src/styles/global.css`）
//! 生成到 [`tokens`]；本模块只提供结构、全局安装与读取。
//!
//! # 与 zed `theme` crate 的关系
//!
//! zed 的 `ui` 组件依赖 zed `theme`（由 [`crate::init_theme`] 安装，保持不动）。
//! Buddy 自己的界面**只读** [`Theme`]（`cx.buddy_theme()`），不读 zed 主题色 ——
//! 否则会混入 zed 的配色，偏离 v1 观感。
//!
//! # 外观只有浅 / 深两种
//!
//! v1 的主题设置只有 `light` / `dark`（`src/types/index.ts` `Theme`、engine `models::Theme`），
//! **不跟随系统外观**。v2 保持一致；是否新增「跟随系统」由用户决定（S03-06 决策记录）。

pub mod tokens;

use buddy_engine::models;
use gpui::{App, BoxShadow, Global, Hsla, Rgba, point, px};
use tokens::{Palette, Shadows};

/// 一层阴影（`box-shadow` / `drop-shadow` 的一项），单位为逻辑像素。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowSpec {
    /// 水平偏移
    pub x: f32,
    /// 垂直偏移
    pub y: f32,
    /// 模糊半径
    pub blur: f32,
    /// 扩展半径
    pub spread: f32,
    /// 颜色
    pub color: Rgba,
    /// 是否内阴影（GPUI `BoxShadow::inset` 原生支持）
    pub inset: bool,
}

impl ShadowSpec {
    /// 转为 GPUI 的 `BoxShadow`
    pub fn to_box_shadow(&self) -> BoxShadow {
        BoxShadow {
            color: Hsla::from(self.color),
            offset: point(px(self.x), px(self.y)),
            blur_radius: px(self.blur),
            spread_radius: px(self.spread),
            inset: self.inset,
        }
    }
}

/// 把整组阴影转为 GPUI 可直接使用的列表（`div().shadow(..)`）
pub fn box_shadows(layers: &[ShadowSpec]) -> Vec<BoxShadow> {
    layers.iter().map(ShadowSpec::to_box_shadow).collect()
}

/// 外观
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Appearance {
    /// 浅色（v1 默认）
    #[default]
    Light,
    /// 深色
    Dark,
}

impl From<models::Theme> for Appearance {
    fn from(theme: models::Theme) -> Self {
        match theme {
            models::Theme::Light => Self::Light,
            models::Theme::Dark => Self::Dark,
        }
    }
}

impl From<Appearance> for models::Theme {
    fn from(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self::Light,
            Appearance::Dark => Self::Dark,
        }
    }
}

/// 当前生效的主题：外观 + 对应的令牌
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// 外观
    pub appearance: Appearance,
    /// 颜色
    pub colors: &'static Palette,
    /// 阴影与发光
    pub shadows: &'static Shadows,
}

impl Global for Theme {}

impl Theme {
    /// 取某个外观的主题（纯数据，不安装）
    pub fn of(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self { appearance, colors: &tokens::LIGHT, shadows: &tokens::LIGHT_SHADOWS },
            Appearance::Dark => Self { appearance, colors: &tokens::DARK, shadows: &tokens::DARK_SHADOWS },
        }
    }

    /// 安装为全局主题；已打开的窗口在下一帧重绘时读取新值（切换见 [`set_appearance`]）
    pub fn install(appearance: Appearance, cx: &mut App) {
        cx.set_global(Self::of(appearance));
    }
}

/// 运行时切换外观：更新全局主题并刷新所有窗口（S03-06）
pub fn set_appearance(appearance: Appearance, cx: &mut App) {
    Theme::install(appearance, cx);
    cx.refresh_windows();
}

/// 读取 Buddy 主题：`cx.buddy_theme()`
pub trait BuddyTheme {
    /// 当前主题（未安装时 panic —— 启动序列必须先 [`Theme::install`]）
    fn buddy_theme(&self) -> &Theme;
}

impl BuddyTheme for App {
    fn buddy_theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(c: Rgba) -> String {
        format!("#{:02X}{:02X}{:02X}", (c.r * 255.0).round() as u8, (c.g * 255.0).round() as u8, (c.b * 255.0).round() as u8)
    }

    #[test]
    fn of_selects_the_matching_token_set() {
        let light = Theme::of(Appearance::Light);
        let dark = Theme::of(Appearance::Dark);
        assert_eq!(light.appearance, Appearance::Light);
        assert_eq!(dark.appearance, Appearance::Dark);
        // v1 global.css：--bg-canvas 浅 #F3F1EE / 深 #181719
        assert_eq!(hex(light.colors.bg_canvas), "#F3F1EE");
        assert_eq!(hex(dark.colors.bg_canvas), "#181719");
        // 品牌色两套主题相同（硬约束 2）
        assert_eq!(hex(light.colors.buddy_primary), "#5B5FE9");
        assert_eq!(light.colors.buddy_primary, dark.colors.buddy_primary);
    }

    #[test]
    fn appearance_round_trips_with_config_theme() {
        for a in [Appearance::Light, Appearance::Dark] {
            assert_eq!(Appearance::from(models::Theme::from(a)), a);
        }
        assert_eq!(Appearance::default(), Appearance::Light);
    }

    #[test]
    fn shadow_spec_keeps_every_field_including_inset() {
        // --shadow-window 浅色第 3 层为 `0 1px 0 rgba(255,255,255,0.55) inset`
        let layers = Theme::of(Appearance::Light).shadows.shadow_window;
        assert_eq!(layers.len(), 3);
        let s = layers[2].to_box_shadow();
        assert!(s.inset);
        assert_eq!(s.offset.y, px(1.0));
        assert!((s.color.a - 0.55).abs() < 1e-6);
        assert!(!layers[0].to_box_shadow().inset);
    }

    #[test]
    fn excluded_tokens_are_accounted_for() {
        assert_eq!(tokens::EXCLUDED.len(), 1);
        assert_eq!(tokens::EXCLUDED[0].0, "--glass-outline");
        assert_eq!(tokens::SOURCE_TOKEN_COUNT, 139);
    }
}
