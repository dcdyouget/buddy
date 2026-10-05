//! 设计令牌 —— **界面颜色、间距、圆角的唯一来源**，改外观直接改这里。
//!
//! 最初由 v1 的 `src/styles/global.css`（git 标签 `v1-final`）逐变量生成（139 个变量，
//! 1 个按理由排除，见 [`EXCLUDED`]）。每个颜色后的注释为 `#RRGGBBAA`，便于人工复核。

#![allow(missing_docs)] // 字段即 CSS 变量名，逐个写文档无信息量

use super::ShadowSpec;
use gpui::Rgba;

/// 源 CSS 变量总数（完备性校验：生成项 + 排除项 = 此数）
pub const SOURCE_TOKEN_COUNT: usize = 139;

/// 不迁移的令牌及理由
pub const EXCLUDED: &[(&str, &str)] = &[
    ("--glass-outline", "用户实机否决（白色描边观感为「四边白光」），v2 不迁移"),
];

/// 随主题变化的颜色（浅 / 深各一份）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// `--bg-canvas`
    pub bg_canvas: Rgba,
    /// `--bg-elevated`
    pub bg_elevated: Rgba,
    /// `--bg-overlay`
    pub bg_overlay: Rgba,
    /// `--bg-sunken`
    pub bg_sunken: Rgba,
    /// `--bg-surface`
    pub bg_surface: Rgba,
    /// `--border-default`
    pub border_default: Rgba,
    /// `--border-strong`
    pub border_strong: Rgba,
    /// `--border-subtle`
    pub border_subtle: Rgba,
    /// `--buddy-primary`
    pub buddy_primary: Rgba,
    /// `--buddy-primary-100`
    pub buddy_primary_100: Rgba,
    /// `--buddy-primary-200`
    pub buddy_primary_200: Rgba,
    /// `--buddy-primary-300`
    pub buddy_primary_300: Rgba,
    /// `--buddy-primary-400`
    pub buddy_primary_400: Rgba,
    /// `--buddy-primary-50`
    pub buddy_primary_50: Rgba,
    /// `--buddy-primary-500`
    pub buddy_primary_500: Rgba,
    /// `--buddy-primary-600`
    pub buddy_primary_600: Rgba,
    /// `--buddy-primary-700`
    pub buddy_primary_700: Rgba,
    /// `--buddy-primary-800`
    pub buddy_primary_800: Rgba,
    /// `--buddy-primary-900`
    pub buddy_primary_900: Rgba,
    /// `--code-bg`
    pub code_bg: Rgba,
    /// `--code-border`
    pub code_border: Rgba,
    /// `--code-header-bg`
    pub code_header_bg: Rgba,
    /// `--code-syntax-comment`
    pub code_syntax_comment: Rgba,
    /// `--code-syntax-function`
    pub code_syntax_function: Rgba,
    /// `--code-syntax-keyword`
    pub code_syntax_keyword: Rgba,
    /// `--code-syntax-number`
    pub code_syntax_number: Rgba,
    /// `--code-syntax-operator`
    pub code_syntax_operator: Rgba,
    /// `--code-syntax-property`
    pub code_syntax_property: Rgba,
    /// `--code-syntax-punctuation`
    pub code_syntax_punctuation: Rgba,
    /// `--code-syntax-special`
    pub code_syntax_special: Rgba,
    /// `--code-syntax-string`
    pub code_syntax_string: Rgba,
    /// `--code-text`
    pub code_text: Rgba,
    /// `--composer-glow`
    pub composer_glow: Rgba,
    /// `--composer-surface`
    pub composer_surface: Rgba,
    /// `--control-surface`
    pub control_surface: Rgba,
    /// `--field-surface`
    pub field_surface: Rgba,
    /// `--markdown-accent`
    pub markdown_accent: Rgba,
    /// `--markdown-accent-line`
    pub markdown_accent_line: Rgba,
    /// `--markdown-accent-medium`
    pub markdown_accent_medium: Rgba,
    /// `--markdown-accent-soft`
    pub markdown_accent_soft: Rgba,
    /// `--markdown-accent-strong`
    pub markdown_accent_strong: Rgba,
    /// `--neutral-0`
    pub neutral_0: Rgba,
    /// `--neutral-100`
    pub neutral_100: Rgba,
    /// `--neutral-1000`
    pub neutral_1000: Rgba,
    /// `--neutral-200`
    pub neutral_200: Rgba,
    /// `--neutral-300`
    pub neutral_300: Rgba,
    /// `--neutral-400`
    pub neutral_400: Rgba,
    /// `--neutral-50`
    pub neutral_50: Rgba,
    /// `--neutral-500`
    pub neutral_500: Rgba,
    /// `--neutral-600`
    pub neutral_600: Rgba,
    /// `--neutral-700`
    pub neutral_700: Rgba,
    /// `--neutral-800`
    pub neutral_800: Rgba,
    /// `--neutral-900`
    pub neutral_900: Rgba,
    /// `--panel-surface`
    pub panel_surface: Rgba,
    /// `--primary-tint-soft`
    pub primary_tint_soft: Rgba,
    /// `--primary-tint-strong`
    pub primary_tint_strong: Rgba,
    /// `--state-error`
    pub state_error: Rgba,
    /// `--state-info`
    pub state_info: Rgba,
    /// `--state-success`
    pub state_success: Rgba,
    /// `--state-warning`
    pub state_warning: Rgba,
    /// `--streaming-star-blue`
    pub streaming_star_blue: Rgba,
    /// `--streaming-star-blue-soft`
    pub streaming_star_blue_soft: Rgba,
    /// `--streaming-star-white`
    pub streaming_star_white: Rgba,
    /// `--surface-highlight`
    pub surface_highlight: Rgba,
    /// `--text-muted`
    pub text_muted: Rgba,
    /// `--text-on-primary`
    pub text_on_primary: Rgba,
    /// `--text-primary`
    pub text_primary: Rgba,
    /// `--text-tertiary`
    pub text_tertiary: Rgba,
    /// `--tool-ui-accent`
    pub tool_ui_accent: Rgba,
    /// `--tool-ui-accent-medium`
    pub tool_ui_accent_medium: Rgba,
    /// `--tool-ui-accent-soft`
    pub tool_ui_accent_soft: Rgba,
    /// `--tool-ui-accent-strong`
    pub tool_ui_accent_strong: Rgba,
    /// `--tool-ui-action`
    pub tool_ui_action: Rgba,
    /// `--tool-ui-flow-highlight`
    pub tool_ui_flow_highlight: Rgba,
    /// `--tool-ui-flow-secondary`
    pub tool_ui_flow_secondary: Rgba,
    /// `--user-bubble`
    pub user_bubble: Rgba,
    /// `--user-bubble-border`
    pub user_bubble_border: Rgba,
    /// `--window-inner-highlight`
    pub window_inner_highlight: Rgba,
    /// `--window-outline`
    pub window_outline: Rgba,
}

impl Palette {
    /// 全部颜色（CSS 变量名, 值），按变量名排序 —— 供预览与校验遍历
    pub fn entries(&self) -> Vec<(&'static str, Rgba)> {
        vec![
            ("--bg-canvas", self.bg_canvas),
            ("--bg-elevated", self.bg_elevated),
            ("--bg-overlay", self.bg_overlay),
            ("--bg-sunken", self.bg_sunken),
            ("--bg-surface", self.bg_surface),
            ("--border-default", self.border_default),
            ("--border-strong", self.border_strong),
            ("--border-subtle", self.border_subtle),
            ("--buddy-primary", self.buddy_primary),
            ("--buddy-primary-100", self.buddy_primary_100),
            ("--buddy-primary-200", self.buddy_primary_200),
            ("--buddy-primary-300", self.buddy_primary_300),
            ("--buddy-primary-400", self.buddy_primary_400),
            ("--buddy-primary-50", self.buddy_primary_50),
            ("--buddy-primary-500", self.buddy_primary_500),
            ("--buddy-primary-600", self.buddy_primary_600),
            ("--buddy-primary-700", self.buddy_primary_700),
            ("--buddy-primary-800", self.buddy_primary_800),
            ("--buddy-primary-900", self.buddy_primary_900),
            ("--code-bg", self.code_bg),
            ("--code-border", self.code_border),
            ("--code-header-bg", self.code_header_bg),
            ("--code-syntax-comment", self.code_syntax_comment),
            ("--code-syntax-function", self.code_syntax_function),
            ("--code-syntax-keyword", self.code_syntax_keyword),
            ("--code-syntax-number", self.code_syntax_number),
            ("--code-syntax-operator", self.code_syntax_operator),
            ("--code-syntax-property", self.code_syntax_property),
            ("--code-syntax-punctuation", self.code_syntax_punctuation),
            ("--code-syntax-special", self.code_syntax_special),
            ("--code-syntax-string", self.code_syntax_string),
            ("--code-text", self.code_text),
            ("--composer-glow", self.composer_glow),
            ("--composer-surface", self.composer_surface),
            ("--control-surface", self.control_surface),
            ("--field-surface", self.field_surface),
            ("--markdown-accent", self.markdown_accent),
            ("--markdown-accent-line", self.markdown_accent_line),
            ("--markdown-accent-medium", self.markdown_accent_medium),
            ("--markdown-accent-soft", self.markdown_accent_soft),
            ("--markdown-accent-strong", self.markdown_accent_strong),
            ("--neutral-0", self.neutral_0),
            ("--neutral-100", self.neutral_100),
            ("--neutral-1000", self.neutral_1000),
            ("--neutral-200", self.neutral_200),
            ("--neutral-300", self.neutral_300),
            ("--neutral-400", self.neutral_400),
            ("--neutral-50", self.neutral_50),
            ("--neutral-500", self.neutral_500),
            ("--neutral-600", self.neutral_600),
            ("--neutral-700", self.neutral_700),
            ("--neutral-800", self.neutral_800),
            ("--neutral-900", self.neutral_900),
            ("--panel-surface", self.panel_surface),
            ("--primary-tint-soft", self.primary_tint_soft),
            ("--primary-tint-strong", self.primary_tint_strong),
            ("--state-error", self.state_error),
            ("--state-info", self.state_info),
            ("--state-success", self.state_success),
            ("--state-warning", self.state_warning),
            ("--streaming-star-blue", self.streaming_star_blue),
            ("--streaming-star-blue-soft", self.streaming_star_blue_soft),
            ("--streaming-star-white", self.streaming_star_white),
            ("--surface-highlight", self.surface_highlight),
            ("--text-muted", self.text_muted),
            ("--text-on-primary", self.text_on_primary),
            ("--text-primary", self.text_primary),
            ("--text-tertiary", self.text_tertiary),
            ("--tool-ui-accent", self.tool_ui_accent),
            ("--tool-ui-accent-medium", self.tool_ui_accent_medium),
            ("--tool-ui-accent-soft", self.tool_ui_accent_soft),
            ("--tool-ui-accent-strong", self.tool_ui_accent_strong),
            ("--tool-ui-action", self.tool_ui_action),
            ("--tool-ui-flow-highlight", self.tool_ui_flow_highlight),
            ("--tool-ui-flow-secondary", self.tool_ui_flow_secondary),
            ("--user-bubble", self.user_bubble),
            ("--user-bubble-border", self.user_bubble_border),
            ("--window-inner-highlight", self.window_inner_highlight),
            ("--window-outline", self.window_outline),
        ]
    }
}

pub const LIGHT: Palette = Palette {
    bg_canvas: Rgba { r: 0.952941, g: 0.945098, b: 0.933333, a: 1.000000 }, // #F3F1EEFF
    bg_elevated: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    bg_overlay: Rgba { r: 0.109804, g: 0.098039, b: 0.090196, a: 0.380000 }, // #1C191761
    bg_sunken: Rgba { r: 0.168627, g: 0.156863, b: 0.149020, a: 0.055000 }, // #2B28260E
    bg_surface: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    border_default: Rgba { r: 0.168627, g: 0.156863, b: 0.149020, a: 0.100000 }, // #2B28261A
    border_strong: Rgba { r: 0.168627, g: 0.156863, b: 0.149020, a: 0.160000 }, // #2B282629
    border_subtle: Rgba { r: 0.168627, g: 0.156863, b: 0.149020, a: 0.065000 }, // #2B282611
    buddy_primary: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 1.000000 }, // #5B5FE9FF
    buddy_primary_100: Rgba { r: 0.886275, g: 0.890196, b: 0.988235, a: 1.000000 }, // #E2E3FCFF
    buddy_primary_200: Rgba { r: 0.776471, g: 0.780392, b: 0.972549, a: 1.000000 }, // #C6C7F8FF
    buddy_primary_300: Rgba { r: 0.662745, g: 0.670588, b: 0.952941, a: 1.000000 }, // #A9ABF3FF
    buddy_primary_400: Rgba { r: 0.552941, g: 0.560784, b: 0.933333, a: 1.000000 }, // #8D8FEEFF
    buddy_primary_50: Rgba { r: 0.945098, g: 0.945098, b: 0.996078, a: 1.000000 }, // #F1F1FEFF
    buddy_primary_500: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 1.000000 }, // #5B5FE9FF
    buddy_primary_600: Rgba { r: 0.290196, g: 0.305882, b: 0.831373, a: 1.000000 }, // #4A4ED4FF
    buddy_primary_700: Rgba { r: 0.239216, g: 0.250980, b: 0.690196, a: 1.000000 }, // #3D40B0FF
    buddy_primary_800: Rgba { r: 0.184314, g: 0.196078, b: 0.541176, a: 1.000000 }, // #2F328AFF
    buddy_primary_900: Rgba { r: 0.121569, g: 0.129412, b: 0.376471, a: 1.000000 }, // #1F2160FF
    code_bg: Rgba { r: 0.965804, g: 0.975529, b: 0.996863, a: 1.000000 }, // #F6F9FEFF
    code_border: Rgba { r: 0.829020, g: 0.877647, b: 0.984314, a: 1.000000 }, // #D3E0FBFF
    code_header_bg: Rgba { r: 0.940157, g: 0.957176, b: 0.994510, a: 1.000000 }, // #F0F4FEFF
    code_syntax_comment: Rgba { r: 0.428078, g: 0.458980, b: 0.545333, a: 1.000000 }, // #6D758BFF
    code_syntax_function: Rgba { r: 0.126275, g: 0.468549, b: 0.719529, a: 1.000000 }, // #2077B7FF
    code_syntax_keyword: Rgba { r: 0.143451, g: 0.352000, b: 0.810118, a: 1.000000 }, // #255ACFFF
    code_syntax_number: Rgba { r: 0.693098, g: 0.392471, b: 0.045961, a: 1.000000 }, // #B1640CFF
    code_syntax_operator: Rgba { r: 0.139451, g: 0.264000, b: 0.539451, a: 1.000000 }, // #24438AFF
    code_syntax_property: Rgba { r: 0.142039, g: 0.320941, b: 0.714588, a: 1.000000 }, // #2452B6FF
    code_syntax_punctuation: Rgba { r: 0.368627, g: 0.356863, b: 0.349020, a: 1.000000 }, // #5E5B59FF
    code_syntax_special: Rgba { r: 0.610980, g: 0.440000, b: 0.328863, a: 1.000000 }, // #9C7054FF
    code_syntax_string: Rgba { r: 0.107451, g: 0.548863, b: 0.517490, a: 1.000000 }, // #1B8C84FF
    code_text: Rgba { r: 0.134510, g: 0.155294, b: 0.205098, a: 1.000000 }, // #222834FF
    composer_glow: Rgba { r: 0.555451, g: 0.681882, b: 0.959216, a: 1.000000 }, // #8EAEF5FF
    composer_surface: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    control_surface: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    field_surface: Rgba { r: 0.168627, g: 0.156863, b: 0.149020, a: 0.045000 }, // #2B28260B
    markdown_accent: Rgba { r: 0.241725, g: 0.412392, b: 0.793098, a: 1.000000 }, // #3E69CAFF
    markdown_accent_line: Rgba { r: 0.241725, g: 0.412392, b: 0.793098, a: 0.380000 }, // #3E69CA61
    markdown_accent_medium: Rgba { r: 0.241725, g: 0.412392, b: 0.793098, a: 0.240000 }, // #3E69CA3D
    markdown_accent_soft: Rgba { r: 0.241725, g: 0.412392, b: 0.793098, a: 0.100000 }, // #3E69CA1A
    markdown_accent_strong: Rgba { r: 0.142980, g: 0.341647, b: 0.778275, a: 1.000000 }, // #2457C6FF
    neutral_0: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    neutral_100: Rgba { r: 0.960784, g: 0.956863, b: 0.949020, a: 1.000000 }, // #F5F4F2FF
    neutral_1000: Rgba { r: 0.070588, g: 0.066667, b: 0.062745, a: 1.000000 }, // #121110FF
    neutral_200: Rgba { r: 0.913725, g: 0.905882, b: 0.894118, a: 1.000000 }, // #E9E7E4FF
    neutral_300: Rgba { r: 0.831373, g: 0.819608, b: 0.803922, a: 1.000000 }, // #D4D1CDFF
    neutral_400: Rgba { r: 0.650980, g: 0.635294, b: 0.615686, a: 1.000000 }, // #A6A29DFF
    neutral_50: Rgba { r: 0.984314, g: 0.980392, b: 0.976471, a: 1.000000 }, // #FBFAF9FF
    neutral_500: Rgba { r: 0.490196, g: 0.474510, b: 0.462745, a: 1.000000 }, // #7D7976FF
    neutral_600: Rgba { r: 0.368627, g: 0.356863, b: 0.349020, a: 1.000000 }, // #5E5B59FF
    neutral_700: Rgba { r: 0.270588, g: 0.258824, b: 0.250980, a: 1.000000 }, // #454240FF
    neutral_800: Rgba { r: 0.188235, g: 0.180392, b: 0.176471, a: 1.000000 }, // #302E2DFF
    neutral_900: Rgba { r: 0.133333, g: 0.129412, b: 0.125490, a: 1.000000 }, // #222120FF
    panel_surface: Rgba { r: 0.960784, g: 0.956863, b: 0.949020, a: 1.000000 }, // #F5F4F2FF
    primary_tint_soft: Rgba { r: 0.945098, g: 0.945098, b: 0.996078, a: 1.000000 }, // #F1F1FEFF
    primary_tint_strong: Rgba { r: 0.886275, g: 0.890196, b: 0.988235, a: 1.000000 }, // #E2E3FCFF
    state_error: Rgba { r: 0.862745, g: 0.149020, b: 0.149020, a: 1.000000 }, // #DC2626FF
    state_info: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 1.000000 }, // #2563EBFF
    state_success: Rgba { r: 0.086275, g: 0.639216, b: 0.290196, a: 1.000000 }, // #16A34AFF
    state_warning: Rgba { r: 0.850980, g: 0.466667, b: 0.023529, a: 1.000000 }, // #D97706FF
    streaming_star_blue: Rgba { r: 0.298980, g: 0.498353, b: 0.935686, a: 1.000000 }, // #4C7FEFFF
    streaming_star_blue_soft: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 0.520000 }, // #2563EB85
    streaming_star_white: Rgba { r: 0.931608, g: 0.951059, b: 0.993725, a: 1.000000 }, // #EEF3FDFF
    surface_highlight: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.000000 }, // #00000000
    text_muted: Rgba { r: 0.368627, g: 0.356863, b: 0.349020, a: 1.000000 }, // #5E5B59FF
    text_on_primary: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    text_primary: Rgba { r: 0.133333, g: 0.129412, b: 0.125490, a: 1.000000 }, // #222120FF
    text_tertiary: Rgba { r: 0.490196, g: 0.474510, b: 0.462745, a: 1.000000 }, // #7D7976FF
    tool_ui_accent: Rgba { r: 0.241725, g: 0.412392, b: 0.793098, a: 1.000000 }, // #3E69CAFF
    tool_ui_accent_medium: Rgba { r: 0.241725, g: 0.412392, b: 0.793098, a: 0.240000 }, // #3E69CA3D
    tool_ui_accent_soft: Rgba { r: 0.241725, g: 0.412392, b: 0.793098, a: 0.100000 }, // #3E69CA1A
    tool_ui_accent_strong: Rgba { r: 0.142980, g: 0.341647, b: 0.778275, a: 1.000000 }, // #2457C6FF
    tool_ui_action: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 1.000000 }, // #2563EBFF
    tool_ui_flow_highlight: Rgba { r: 0.589647, g: 0.706353, b: 0.962353, a: 1.000000 }, // #96B4F5FF
    tool_ui_flow_secondary: Rgba { r: 0.126275, g: 0.468549, b: 0.719529, a: 1.000000 }, // #2077B7FF
    user_bubble: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 0.075000 }, // #5B5FE913
    user_bubble_border: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 0.140000 }, // #5B5FE924
    window_inner_highlight: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.720000 }, // #FFFFFFB8
    window_outline: Rgba { r: 0.797569, g: 0.788471, b: 0.777098, a: 1.000000 }, // #CBC9C6FF
};

pub const DARK: Palette = Palette {
    bg_canvas: Rgba { r: 0.094118, g: 0.090196, b: 0.098039, a: 1.000000 }, // #181719FF
    bg_elevated: Rgba { r: 0.188235, g: 0.180392, b: 0.200000, a: 1.000000 }, // #302E33FF
    bg_overlay: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.550000 }, // #0000008C
    bg_sunken: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.065000 }, // #FFFFFF11
    bg_surface: Rgba { r: 0.109804, g: 0.105882, b: 0.117647, a: 1.000000 }, // #1C1B1EFF
    border_default: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.130000 }, // #FFFFFF21
    border_strong: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.190000 }, // #FFFFFF30
    border_subtle: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.085000 }, // #FFFFFF16
    buddy_primary: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 1.000000 }, // #5B5FE9FF
    buddy_primary_100: Rgba { r: 0.886275, g: 0.890196, b: 0.988235, a: 1.000000 }, // #E2E3FCFF
    buddy_primary_200: Rgba { r: 0.776471, g: 0.780392, b: 0.972549, a: 1.000000 }, // #C6C7F8FF
    buddy_primary_300: Rgba { r: 0.662745, g: 0.670588, b: 0.952941, a: 1.000000 }, // #A9ABF3FF
    buddy_primary_400: Rgba { r: 0.552941, g: 0.560784, b: 0.933333, a: 1.000000 }, // #8D8FEEFF
    buddy_primary_50: Rgba { r: 0.945098, g: 0.945098, b: 0.996078, a: 1.000000 }, // #F1F1FEFF
    buddy_primary_500: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 1.000000 }, // #5B5FE9FF
    buddy_primary_600: Rgba { r: 0.290196, g: 0.305882, b: 0.831373, a: 1.000000 }, // #4A4ED4FF
    buddy_primary_700: Rgba { r: 0.239216, g: 0.250980, b: 0.690196, a: 1.000000 }, // #3D40B0FF
    buddy_primary_800: Rgba { r: 0.184314, g: 0.196078, b: 0.541176, a: 1.000000 }, // #2F328AFF
    buddy_primary_900: Rgba { r: 0.121569, g: 0.129412, b: 0.376471, a: 1.000000 }, // #1F2160FF
    code_bg: Rgba { r: 0.070588, g: 0.066667, b: 0.078431, a: 1.000000 }, // #121114FF
    code_border: Rgba { r: 0.092941, g: 0.163137, b: 0.331373, a: 1.000000 }, // #182A54FF
    code_header_bg: Rgba { r: 0.079529, g: 0.105255, b: 0.179608, a: 1.000000 }, // #141B2EFF
    code_syntax_comment: Rgba { r: 0.475451, g: 0.516706, b: 0.624863, a: 1.000000 }, // #79849FFF
    code_syntax_function: Rgba { r: 0.114510, g: 0.518745, b: 0.593255, a: 1.000000 }, // #1D8497FF
    code_syntax_keyword: Rgba { r: 0.469961, g: 0.620706, b: 0.951373, a: 1.000000 }, // #789EF3FF
    code_syntax_number: Rgba { r: 0.901647, g: 0.648000, b: 0.355529, a: 1.000000 }, // #E6A55BFF
    code_syntax_operator: Rgba { r: 0.692235, g: 0.779765, b: 0.971765, a: 1.000000 }, // #B1C7F8FF
    code_syntax_property: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 1.000000 }, // #81A5F3FF
    code_syntax_punctuation: Rgba { r: 0.717647, g: 0.701961, b: 0.690196, a: 1.000000 }, // #B7B3B0FF
    code_syntax_special: Rgba { r: 0.922510, g: 0.722667, b: 0.492235, a: 1.000000 }, // #EBB87EFF
    code_syntax_string: Rgba { r: 0.470039, g: 0.790745, b: 0.588314, a: 1.000000 }, // #78CA96FF
    code_text: Rgba { r: 0.846118, g: 0.889882, b: 0.985882, a: 1.000000 }, // #D8E3FBFF
    composer_glow: Rgba { r: 0.212863, g: 0.383216, b: 0.919059, a: 1.000000 }, // #3662EAFF
    composer_surface: Rgba { r: 0.152941, g: 0.149020, b: 0.164706, a: 1.000000 }, // #27262AFF
    control_surface: Rgba { r: 0.188235, g: 0.180392, b: 0.200000, a: 1.000000 }, // #302E33FF
    field_surface: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.055000 }, // #FFFFFF0E
    markdown_accent: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 1.000000 }, // #81A5F3FF
    markdown_accent_line: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 0.440000 }, // #81A5F370
    markdown_accent_medium: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 0.280000 }, // #81A5F347
    markdown_accent_soft: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 0.130000 }, // #81A5F321
    markdown_accent_strong: Rgba { r: 0.640941, g: 0.743059, b: 0.967059, a: 1.000000 }, // #A3BDF7FF
    neutral_0: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    neutral_100: Rgba { r: 0.960784, g: 0.956863, b: 0.949020, a: 1.000000 }, // #F5F4F2FF
    neutral_1000: Rgba { r: 0.070588, g: 0.066667, b: 0.062745, a: 1.000000 }, // #121110FF
    neutral_200: Rgba { r: 0.913725, g: 0.905882, b: 0.894118, a: 1.000000 }, // #E9E7E4FF
    neutral_300: Rgba { r: 0.831373, g: 0.819608, b: 0.803922, a: 1.000000 }, // #D4D1CDFF
    neutral_400: Rgba { r: 0.650980, g: 0.635294, b: 0.615686, a: 1.000000 }, // #A6A29DFF
    neutral_50: Rgba { r: 0.984314, g: 0.980392, b: 0.976471, a: 1.000000 }, // #FBFAF9FF
    neutral_500: Rgba { r: 0.490196, g: 0.474510, b: 0.462745, a: 1.000000 }, // #7D7976FF
    neutral_600: Rgba { r: 0.368627, g: 0.356863, b: 0.349020, a: 1.000000 }, // #5E5B59FF
    neutral_700: Rgba { r: 0.270588, g: 0.258824, b: 0.250980, a: 1.000000 }, // #454240FF
    neutral_800: Rgba { r: 0.188235, g: 0.180392, b: 0.176471, a: 1.000000 }, // #302E2DFF
    neutral_900: Rgba { r: 0.133333, g: 0.129412, b: 0.125490, a: 1.000000 }, // #222120FF
    panel_surface: Rgba { r: 0.188235, g: 0.180392, b: 0.200000, a: 1.000000 }, // #302E33FF
    primary_tint_soft: Rgba { r: 0.439216, g: 0.458824, b: 1.000000, a: 0.180000 }, // #7075FF2E
    primary_tint_strong: Rgba { r: 0.439216, g: 0.458824, b: 1.000000, a: 0.300000 }, // #7075FF4C
    state_error: Rgba { r: 0.862745, g: 0.149020, b: 0.149020, a: 1.000000 }, // #DC2626FF
    state_info: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 1.000000 }, // #2563EBFF
    state_success: Rgba { r: 0.086275, g: 0.639216, b: 0.290196, a: 1.000000 }, // #16A34AFF
    state_warning: Rgba { r: 0.850980, g: 0.466667, b: 0.023529, a: 1.000000 }, // #D97706FF
    streaming_star_blue: Rgba { r: 0.298980, g: 0.498353, b: 0.935686, a: 1.000000 }, // #4C7FEFFF
    streaming_star_blue_soft: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 0.520000 }, // #2563EB85
    streaming_star_white: Rgba { r: 0.931608, g: 0.951059, b: 0.993725, a: 1.000000 }, // #EEF3FDFF
    surface_highlight: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.045000 }, // #FFFFFF0B
    text_muted: Rgba { r: 0.717647, g: 0.701961, b: 0.690196, a: 1.000000 }, // #B7B3B0FF
    text_on_primary: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, // #FFFFFFFF
    text_primary: Rgba { r: 0.968627, g: 0.964706, b: 0.960784, a: 1.000000 }, // #F7F6F5FF
    text_tertiary: Rgba { r: 0.568627, g: 0.552941, b: 0.541176, a: 1.000000 }, // #918D8AFF
    tool_ui_accent: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 1.000000 }, // #81A5F3FF
    tool_ui_accent_medium: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 0.280000 }, // #81A5F347
    tool_ui_accent_soft: Rgba { r: 0.504157, g: 0.645176, b: 0.954510, a: 0.130000 }, // #81A5F321
    tool_ui_accent_strong: Rgba { r: 0.640941, g: 0.743059, b: 0.967059, a: 1.000000 }, // #A3BDF7FF
    tool_ui_action: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 1.000000 }, // #2563EBFF
    tool_ui_flow_highlight: Rgba { r: 0.589647, g: 0.706353, b: 0.962353, a: 1.000000 }, // #96B4F5FF
    tool_ui_flow_secondary: Rgba { r: 0.126275, g: 0.468549, b: 0.719529, a: 1.000000 }, // #2077B7FF
    user_bubble: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 0.190000 }, // #5B5FE930
    user_bubble_border: Rgba { r: 0.662745, g: 0.670588, b: 0.952941, a: 0.220000 }, // #A9ABF338
    // 深色窗口边只比表面亮一级（≈ 白 10% 叠在 bg_surface 上）；原值为近白不透明色，
    // 在深色桌面上形成一圈白边。
    window_inner_highlight: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.050000 }, // #FFFFFF0D
    window_outline: Rgba { r: 0.200000, g: 0.196078, b: 0.211765, a: 1.000000 }, // #333236FF
};

/// 阴影与发光（`box-shadow` / `filter: drop-shadow` 的逐层数据）
#[derive(Clone, Copy, Debug)]
pub struct Shadows {
    /// `--filter-streaming-star`
    pub filter_streaming_star: &'static [ShadowSpec],
    /// `--shadow-composer`
    pub shadow_composer: &'static [ShadowSpec],
    /// `--shadow-floating-md`
    pub shadow_floating_md: &'static [ShadowSpec],
    /// `--shadow-floating-sm`
    pub shadow_floating_sm: &'static [ShadowSpec],
    /// `--shadow-focus`
    pub shadow_focus: &'static [ShadowSpec],
    /// `--shadow-static`
    pub shadow_static: &'static [ShadowSpec],
    /// `--shadow-streaming-char-settle`
    pub shadow_streaming_char_settle: &'static [ShadowSpec],
    /// `--shadow-window`
    pub shadow_window: &'static [ShadowSpec],
    /// `--shadow-window-edge`
    pub shadow_window_edge: &'static [ShadowSpec],
}

pub const LIGHT_SHADOWS: Shadows = Shadows {
    filter_streaming_star: &[
        ShadowSpec { x: 0.0, y: 0.0, blur: 2.0, spread: 0.0, color: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, inset: false }, // #FFFFFFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 5.0, spread: 0.0, color: Rgba { r: 0.298980, g: 0.498353, b: 0.935686, a: 1.000000 }, inset: false }, // #4C7FEFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 9.0, spread: 0.0, color: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 0.520000 }, inset: false }, // #2563EB85
    ],
    shadow_composer: &[
        ShadowSpec { x: 0.0, y: 14.0, blur: 32.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.140000 }, inset: false }, // #231D1824
        ShadowSpec { x: 0.0, y: 3.0, blur: 9.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.060000 }, inset: false }, // #231D180F
    ],
    shadow_floating_md: &[
        ShadowSpec { x: 0.0, y: 22.0, blur: 64.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.160000 }, inset: false }, // #231D1829
        ShadowSpec { x: 0.0, y: 2.0, blur: 10.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.050000 }, inset: false }, // #231D180D
    ],
    shadow_floating_sm: &[
        ShadowSpec { x: 0.0, y: 8.0, blur: 24.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.090000 }, inset: false }, // #231D1817
        ShadowSpec { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.040000 }, inset: false }, // #231D180A
    ],
    shadow_focus: &[
        ShadowSpec { x: 0.0, y: 0.0, blur: 0.0, spread: 3.0, color: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 0.100000 }, inset: false }, // #5B5FE91A
    ],
    shadow_static: &[
        ShadowSpec { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.040000 }, inset: false }, // #231D180A
    ],
    shadow_streaming_char_settle: &[
        ShadowSpec { x: 0.0, y: 0.0, blur: 2.0, spread: 0.0, color: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, inset: false }, // #FFFFFFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 7.0, spread: 0.0, color: Rgba { r: 0.298980, g: 0.498353, b: 0.935686, a: 1.000000 }, inset: false }, // #4C7FEFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 13.0, spread: 0.0, color: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 0.520000 }, inset: false }, // #2563EB85
    ],
    shadow_window: &[
        ShadowSpec { x: 0.0, y: 30.0, blur: 90.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.220000 }, inset: false }, // #231D1838
        ShadowSpec { x: 0.0, y: 10.0, blur: 34.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.120000 }, inset: false }, // #231D181F
        ShadowSpec { x: 0.0, y: 1.0, blur: 0.0, spread: 0.0, color: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.550000 }, inset: true }, // #FFFFFF8C
    ],
    shadow_window_edge: &[
        ShadowSpec { x: 0.0, y: 2.0, blur: 7.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.240000 }, inset: false }, // #231D183D
        ShadowSpec { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.120000 }, inset: false }, // #231D181F
    ],
};

pub const DARK_SHADOWS: Shadows = Shadows {
    filter_streaming_star: &[
        ShadowSpec { x: 0.0, y: 0.0, blur: 2.0, spread: 0.0, color: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, inset: false }, // #FFFFFFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 5.0, spread: 0.0, color: Rgba { r: 0.298980, g: 0.498353, b: 0.935686, a: 1.000000 }, inset: false }, // #4C7FEFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 9.0, spread: 0.0, color: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 0.520000 }, inset: false }, // #2563EB85
    ],
    shadow_composer: &[
        ShadowSpec { x: 0.0, y: 14.0, blur: 32.0, spread: 0.0, color: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.350000 }, inset: false }, // #00000059
        ShadowSpec { x: 0.0, y: 3.0, blur: 9.0, spread: 0.0, color: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.180000 }, inset: false }, // #0000002E
    ],
    shadow_floating_md: &[
        ShadowSpec { x: 0.0, y: 22.0, blur: 64.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.160000 }, inset: false }, // #231D1829
        ShadowSpec { x: 0.0, y: 2.0, blur: 10.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.050000 }, inset: false }, // #231D180D
    ],
    shadow_floating_sm: &[
        ShadowSpec { x: 0.0, y: 8.0, blur: 24.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.090000 }, inset: false }, // #231D1817
        ShadowSpec { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.040000 }, inset: false }, // #231D180A
    ],
    shadow_focus: &[
        ShadowSpec { x: 0.0, y: 0.0, blur: 0.0, spread: 3.0, color: Rgba { r: 0.356863, g: 0.372549, b: 0.913725, a: 0.100000 }, inset: false }, // #5B5FE91A
    ],
    shadow_static: &[
        ShadowSpec { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Rgba { r: 0.137255, g: 0.113725, b: 0.094118, a: 0.040000 }, inset: false }, // #231D180A
    ],
    shadow_streaming_char_settle: &[
        ShadowSpec { x: 0.0, y: 0.0, blur: 2.0, spread: 0.0, color: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 1.000000 }, inset: false }, // #FFFFFFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 7.0, spread: 0.0, color: Rgba { r: 0.298980, g: 0.498353, b: 0.935686, a: 1.000000 }, inset: false }, // #4C7FEFFF
        ShadowSpec { x: 0.0, y: 0.0, blur: 13.0, spread: 0.0, color: Rgba { r: 0.145098, g: 0.388235, b: 0.921569, a: 0.520000 }, inset: false }, // #2563EB85
    ],
    shadow_window: &[
        ShadowSpec { x: 0.0, y: 30.0, blur: 90.0, spread: 0.0, color: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.480000 }, inset: false }, // #0000007A
        ShadowSpec { x: 0.0, y: 10.0, blur: 34.0, spread: 0.0, color: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.340000 }, inset: false }, // #00000057
        ShadowSpec { x: 0.0, y: 1.0, blur: 0.0, spread: 0.0, color: Rgba { r: 1.000000, g: 1.000000, b: 1.000000, a: 0.080000 }, inset: true }, // #FFFFFF14
    ],
    shadow_window_edge: &[
        ShadowSpec { x: 0.0, y: 2.0, blur: 7.0, spread: 0.0, color: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.580000 }, inset: false }, // #00000094
        ShadowSpec { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Rgba { r: 0.000000, g: 0.000000, b: 0.000000, a: 0.320000 }, inset: false }, // #00000052
    ],
};

/// 尺寸：间距 / 圆角 / 字号为逻辑像素；字距为 em 系数；行高为字号倍数；字重为 CSS 数值
pub mod metrics {
    /// `--font-size-2xl: 24px`
    pub const FONT_SIZE_2XL: f32 = 24.0;
    /// `--font-size-base: 13px`
    pub const FONT_SIZE_BASE: f32 = 13.0;
    /// `--font-size-lg: 16px`
    pub const FONT_SIZE_LG: f32 = 16.0;
    /// `--font-size-md: 14px`
    pub const FONT_SIZE_MD: f32 = 14.0;
    /// `--font-size-sm: 12px`
    pub const FONT_SIZE_SM: f32 = 12.0;
    /// `--font-size-xl: 20px`
    pub const FONT_SIZE_XL: f32 = 20.0;
    /// `--font-size-xs: 11px`
    pub const FONT_SIZE_XS: f32 = 11.0;
    /// `--radius-full: 9999px`
    pub const RADIUS_FULL: f32 = 9999.0;
    /// `--radius-lg: 12px`
    pub const RADIUS_LG: f32 = 12.0;
    /// `--radius-md: 8px`
    pub const RADIUS_MD: f32 = 8.0;
    /// `--radius-sm: 4px`
    pub const RADIUS_SM: f32 = 4.0;
    /// `--radius-xl: 16px`
    pub const RADIUS_XL: f32 = 16.0;
    /// `--space-1: 4px`
    pub const SPACE_1: f32 = 4.0;
    /// `--space-10: 40px`
    pub const SPACE_10: f32 = 40.0;
    /// `--space-12: 48px`
    pub const SPACE_12: f32 = 48.0;
    /// `--space-2: 8px`
    pub const SPACE_2: f32 = 8.0;
    /// `--space-3: 12px`
    pub const SPACE_3: f32 = 12.0;
    /// `--space-4: 16px`
    pub const SPACE_4: f32 = 16.0;
    /// `--space-5: 20px`
    pub const SPACE_5: f32 = 20.0;
    /// `--space-6: 24px`
    pub const SPACE_6: f32 = 24.0;
    /// `--space-8: 32px`
    pub const SPACE_8: f32 = 32.0;
    /// `--letter-spacing-base: 0`
    pub const LETTER_SPACING_BASE: f32 = 0.0;
    /// `--letter-spacing-tight: -0.01em`
    pub const LETTER_SPACING_TIGHT: f32 = -0.01;
    /// `--letter-spacing-wide: 0.02em`
    pub const LETTER_SPACING_WIDE: f32 = 0.02;
    /// `--font-weight-emphasis: 750`
    pub const FONT_WEIGHT_EMPHASIS: f32 = 750.0;
    /// `--font-weight-heading: 800`
    pub const FONT_WEIGHT_HEADING: f32 = 800.0;
    /// `--font-weight-regular: 650`
    pub const FONT_WEIGHT_REGULAR: f32 = 650.0;
    /// `--line-height-base: 1.5`
    pub const LINE_HEIGHT_BASE: f32 = 1.5;
    /// `--line-height-relaxed: 1.65`
    pub const LINE_HEIGHT_RELAXED: f32 = 1.65;
    /// `--line-height-tight: 1.25`
    pub const LINE_HEIGHT_TIGHT: f32 = 1.25;
}

/// 动效：时长 / 延迟为毫秒（延迟可为负）；缓动为 cubic-bezier 四参数
pub mod motion {
    /// 呼入：窗口从该缩放比例弹簧放大到原尺寸（macOS 26 聚焦搜索式）。
    pub const SUMMON_START_SCALE: f32 = 0.94;
    /// 呼入弹簧刚度（质量 1）。
    pub const SUMMON_STIFFNESS: f32 = 420.0;
    /// 呼入弹簧阻尼比：略低于临界阻尼，落点干脆且几乎无回弹（回弹会被窗口边界裁切）。
    pub const SUMMON_DAMPING_RATIO: f32 = 0.86;
    /// 呼入淡入时长（毫秒），短于弹簧，先看清再落定。
    pub const DURATION_SUMMON_FADE: u64 = 180;
    /// 呼入视为落定的时长（毫秒）；弹簧尾段肉眼不可见。
    pub const DURATION_SUMMON: u64 = 320;
    /// 呼出：向中心收缩到该比例并淡出。
    pub const DISMISS_END_SCALE: f32 = 0.96;
    /// 呼出时长（毫秒）。
    pub const DURATION_DISMISS: u64 = 150;
    /// Tonal press feedback shared by compact controls.
    pub const BUTTON_PRESS_OPACITY: f32 = 0.88;
    /// Initial opacity for the first settled portion of streaming text.
    pub const STREAMING_SETTLE_START_OPACITY: f32 = 0.66;
    /// Each newly revealed character keeps its own short fade clock.
    pub const DURATION_STREAMING_REVEAL: i32 = 200;
    /// Soft brand-tinted text settles into the normal foreground colour.
    pub const STREAMING_REVEAL_START_OPACITY: f32 = 0.32;
    /// Surface edge light: one quiet pass on interaction, slower while generating.
    pub const DURATION_SURFACE_SHEEN: u64 = 1400;
    /// Repeating surface light period in milliseconds.
    pub const DURATION_SURFACE_FLOW: u64 = 3600;
    /// Fraction of the surface width occupied by the soft edge light.
    pub const SURFACE_SHEEN_WIDTH: f32 = 0.36;
    /// Maximum brand tint opacity along the edge.
    pub const SURFACE_SHEEN_OPACITY: f32 = 0.28;
    /// Soft light falls off within this many pixels of the edge.
    pub const SURFACE_SHEEN_DEPTH: f32 = 10.0;
    /// `--delay-streaming-char-age-1: -32ms`
    pub const DELAY_STREAMING_CHAR_AGE_1: i32 = -32;
    /// `--delay-streaming-char-age-2: -64ms`
    pub const DELAY_STREAMING_CHAR_AGE_2: i32 = -64;
    /// `--delay-streaming-char-age-3: -96ms`
    pub const DELAY_STREAMING_CHAR_AGE_3: i32 = -96;
    /// `--delay-streaming-char-age-4: -128ms`
    pub const DELAY_STREAMING_CHAR_AGE_4: i32 = -128;
    /// `--delay-streaming-char-age-5: -160ms`
    pub const DELAY_STREAMING_CHAR_AGE_5: i32 = -160;
    /// `--delay-streaming-char-age-6: -192ms`
    pub const DELAY_STREAMING_CHAR_AGE_6: i32 = -192;
    /// `--delay-streaming-char-age-7: -224ms`
    pub const DELAY_STREAMING_CHAR_AGE_7: i32 = -224;
    /// `--delay-streaming-char-age-8: -256ms`
    pub const DELAY_STREAMING_CHAR_AGE_8: i32 = -256;
    /// `--duration-fast: 120ms`
    pub const DURATION_FAST: i32 = 120;
    /// `--duration-normal: 200ms`
    pub const DURATION_NORMAL: i32 = 200;
    /// `--duration-slow: 320ms`
    pub const DURATION_SLOW: i32 = 320;
    /// `--duration-streaming-char-settle: 260ms`
    pub const DURATION_STREAMING_CHAR_SETTLE: i32 = 260;
    /// `--duration-streaming-star-breathe: 1.1s`
    pub const DURATION_STREAMING_STAR_BREATHE: i32 = 1100;
    /// `--duration-thinking-loader: 1.2s`
    pub const DURATION_THINKING_LOADER: i32 = 1200;
    /// `--duration-thinking-sheen: 3.4s`
    pub const DURATION_THINKING_SHEEN: i32 = 3400;
    /// `--duration-tool-flow: 2.8s`
    pub const DURATION_TOOL_FLOW: i32 = 2800;
    /// `--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1)`
    pub const EASE_SPRING: [f32; 4] = [0.34, 1.56, 0.64, 1.0];
    /// `--ease-standard: cubic-bezier(0.2, 0.0, 0, 1)`
    pub const EASE_STANDARD: [f32; 4] = [0.2, 0.0, 0.0, 1.0];
}

/// 字体栈（原样保留 CSS 顺序，含通用族名；平台映射见 `theme_system::fonts`）
pub mod fonts {
    /// `--font-mono: 'Fira Code', 'JetBrains Mono', 'SF Mono', 'Menlo', 'Consolas', monospace`
    pub const FONT_MONO: &[&str] = &["Fira Code", "JetBrains Mono", "SF Mono", "Menlo", "Consolas", "monospace"];
    /// `--font-sans: 'Fira Code', 'JetBrains Mono', 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', sans-serif`
    pub const FONT_SANS: &[&str] = &["Fira Code", "JetBrains Mono", "Inter", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "sans-serif"];
}
