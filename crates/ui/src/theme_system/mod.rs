//! Buddy 主题体系
//!
//! 令牌数据在 [`tokens`]（最初取自 v1 `global.css`）；本模块只提供结构、全局安装与读取。
//!
//! # 与 zed `theme` crate 的关系
//!
//! zed 的 `ui` 组件依赖 zed `theme`（由 [`crate::init_theme`] 安装，保持不动）。
//! Buddy 自己的界面**只读** [`Theme`]（`cx.buddy_theme()`），不读 zed 主题色 ——
//! 否则会混入 zed 的配色，偏离 v1 观感。
//!
//! # 阴影的用途（迁自已退役的 `design-tokens.md` §Shadows）
//!
//! | 令牌 | 用于 |
//! |------|------|
//! | `shadow_static` | 输入框、列表项、标签（浅色 alpha ≤ 0.05） |
//! | `shadow_floating_sm` | 设置页内的浮层 |
//! | `shadow_floating_md` | 主窗口、滑入面板 |
//!
//! # 外观只有浅 / 深两种
//!
//! v1 的主题设置只有 `light` / `dark`（`src/types/index.ts` `Theme`、engine `models::Theme`），
//! **不跟随系统外观**。v2 保持一致（2026-09-27 用户决定不做「跟随系统」）。

pub mod easing;
pub mod fonts;
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

/// 排版角色（v1 `global.css` 的 `.t-*` 类，`v1-final` 第 1574–1578 行）
///
/// v1 只定义了这 5 个角色；`design-tokens.md` 所列的 `t-display` / `t-overline` 在代码中**从未存在**。
/// 注意 `title` / `h3` 的字重是 CSS 里写死的 `600`，不是 `--font-weight-*` 令牌。
pub mod typography {
    use super::tokens::metrics as m;

    /// 一个排版角色
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct TextRole {
        /// 字号（逻辑像素）
        pub size: f32,
        /// 字重（CSS 数值）
        pub weight: f32,
        /// 行高（字号倍数）
        pub line_height: f32,
        /// 字距（em 系数）
        pub letter_spacing: f32,
    }

    /// `.t-title`：600 / `--font-size-xl` / `--line-height-tight` / `--letter-spacing-tight`
    pub const TITLE: TextRole = TextRole { size: m::FONT_SIZE_XL, weight: 600.0, line_height: m::LINE_HEIGHT_TIGHT, letter_spacing: m::LETTER_SPACING_TIGHT };
    /// `.t-h3`：600 / `--font-size-lg` / `--line-height-tight`
    pub const H3: TextRole = TextRole { size: m::FONT_SIZE_LG, weight: 600.0, line_height: m::LINE_HEIGHT_TIGHT, letter_spacing: m::LETTER_SPACING_BASE };
    /// `.t-body`：`--font-weight-regular` / `--font-size-md` / `--line-height-base`
    pub const BODY: TextRole = TextRole { size: m::FONT_SIZE_MD, weight: m::FONT_WEIGHT_REGULAR, line_height: m::LINE_HEIGHT_BASE, letter_spacing: m::LETTER_SPACING_BASE };
    /// `.t-body-sm`：`--font-weight-regular` / `--font-size-base` / `--line-height-base`
    pub const BODY_SM: TextRole = TextRole { size: m::FONT_SIZE_BASE, weight: m::FONT_WEIGHT_REGULAR, line_height: m::LINE_HEIGHT_BASE, letter_spacing: m::LETTER_SPACING_BASE };
    /// `.t-caption`：同 body-sm 但 `--font-size-sm`，颜色用 `text_muted`
    pub const CAPTION: TextRole = TextRole { size: m::FONT_SIZE_SM, weight: m::FONT_WEIGHT_REGULAR, line_height: m::LINE_HEIGHT_BASE, letter_spacing: m::LETTER_SPACING_BASE };
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

/// 运行时切换外观：更新全局主题并刷新所有窗口
pub fn set_appearance(appearance: Appearance, cx: &mut App) {
    Theme::install(appearance, cx);
    cx.refresh_windows();
}

/// 对话正文字号（设置页「字体大小」）。代码块、思考块、表格等次级正文随之按差值缩放。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextScale(f32);

impl Global for TextScale {}

impl TextScale {
    /// 当前对话正文字号（逻辑像素）；未安装时为默认 14px。
    pub fn body(cx: &App) -> f32 {
        cx.try_global::<Self>().map_or(models::DEFAULT_FONT_SIZE as f32, |scale| scale.0)
    }

    /// 次级正文（代码 / 思考 / 表格）字号：比正文小 1px，与默认 14 / 13 的令牌关系一致。
    pub fn secondary(cx: &App) -> f32 {
        Self::body(cx) - (tokens::metrics::FONT_SIZE_MD - tokens::metrics::FONT_SIZE_BASE)
    }
}

/// 运行时切换正文字号（钳到允许范围）；字号未变时不刷新窗口。
pub fn set_font_size(size: u32, cx: &mut App) {
    let size = size.clamp(*models::FONT_SIZE_RANGE.start(), *models::FONT_SIZE_RANGE.end()) as f32;
    if TextScale::body(cx) == size && cx.has_global::<TextScale>() {
        return;
    }
    cx.set_global(TextScale(size));
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
        // 品牌色两套主题相同
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
    fn typography_roles_match_v1_classes() {
        use typography::*;
        // v1-final global.css:1574-1578
        assert_eq!((TITLE.size, TITLE.weight), (20.0, 600.0));
        assert_eq!((H3.size, H3.weight), (16.0, 600.0));
        assert_eq!((BODY.size, BODY.weight, BODY.line_height), (14.0, 650.0, 1.5));
        assert_eq!(BODY_SM.size, 13.0);
        assert_eq!(CAPTION.size, 12.0);
        assert_eq!(TITLE.letter_spacing, -0.01);
    }

    #[test]
    fn excluded_tokens_are_accounted_for() {
        assert_eq!(tokens::EXCLUDED.len(), 1);
        assert_eq!(tokens::EXCLUDED[0].0, "--glass-outline");
        assert_eq!(tokens::SOURCE_TOKEN_COUNT, 139);
    }
}
