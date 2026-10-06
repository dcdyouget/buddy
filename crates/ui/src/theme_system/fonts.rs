//! CSS 字体栈 → GPUI `Font`
//!
//! # 为什么要在运行时选字体
//!
//! GPUI 的 `Font.fallbacks` 在 macOS 上交给 CoreText 级联，只在**首选字体缺某个字形**时生效
//! （如 Fira Code 没有中文）；而首选字体**根本没装**时，GPUI 回退到它自己的全局栈
//! （`.ZedMono` / Helvetica …，见 `gpui/src/text_system.rs` `resolve_font`），**不会**沿 CSS 链往下找。
//!
//! WebKit 的行为是「取栈中第一个已安装的字体」。v1 未随包分发字体（`global.css` 无 `@font-face`），
//! 所以 v1 的实际字体取决于用户机器上装了什么。为与 v1 在同一台机器上观感一致，这里照 WebKit 的规则：
//! `family` = 栈中第一个已安装者，`fallbacks` = 其后已安装者（给 CJK 等缺字形时用）。

use super::tokens::fonts;
use gpui::{App, Font, FontFallbacks, Global, TextRenderingMode, font};
use std::collections::HashSet;

/// GPUI 的系统 UI 字体占位名（macOS 映射为 `.AppleSystemUIFont`）
const SYSTEM_UI: &str = ".SystemUIFont";

#[cfg(target_os = "windows")]
const SYSTEM_MONO: &str = "Consolas";
#[cfg(not(target_os = "windows"))]
const SYSTEM_MONO: &str = "Menlo";

/// CSS 族名 → GPUI 族名；`None` 表示 GPUI 无对应（通用族），跳过
fn map_family(css: &str) -> Option<&str> {
    match css {
        "-apple-system" | "BlinkMacSystemFont" | "system-ui" => Some(SYSTEM_UI),
        "sans-serif" | "serif" | "monospace" | "cursive" | "fantasy" => None,
        other => Some(other),
    }
}

/// 按 WebKit 规则从 CSS 字体栈选出 GPUI `Font`（纯函数，便于测试）
///
/// `default_family`：栈中没有任何已安装字体时的兜底（对应 CSS 末尾的通用族）。
pub fn resolve_stack(stack: &[&str], installed: &HashSet<String>, default_family: &str) -> Font {
    // 保序去重：-apple-system 与 BlinkMacSystemFont 都映射为 .SystemUIFont
    let mut available: Vec<&str> = Vec::new();
    for f in stack.iter().filter_map(|f| map_family(f)) {
        if (f == SYSTEM_UI || installed.contains(f)) && !available.contains(&f) {
            available.push(f);
        }
    }
    let (family, rest) = match available.split_first() {
        Some((first, rest)) => (*first, rest),
        None => (default_family, &[][..]),
    };
    let mut f = font(family.to_string());
    if !rest.is_empty() {
        f.fallbacks = Some(FontFallbacks::from_fonts(
            rest.iter().map(|s| s.to_string()).collect(),
        ));
    }
    f
}

/// GPUI 的 `all_font_names()` 会把它内置回退栈的族名（`.ZedMono` / `Segoe UI` / `Ubuntu` …，
/// `gpui/src/text_system.rs` `TextSystem::new`）**无条件并入**结果，不代表本机已安装。
/// CSS 栈中与之重叠的只有 `Segoe UI`（Windows 字体）→ 非 Windows 平台剔除，免得被当作已安装（自检发现）。
#[cfg(not(target_os = "windows"))]
const NOT_INSTALLED_ON_THIS_PLATFORM: &[&str] = &["Segoe UI"];
#[cfg(target_os = "windows")]
const NOT_INSTALLED_ON_THIS_PLATFORM: &[&str] = &[];

fn installed(cx: &App) -> HashSet<String> {
    let mut names: HashSet<String> = cx.text_system().all_font_names().into_iter().collect();
    for n in NOT_INSTALLED_ON_THIS_PLATFORM {
        names.remove(*n);
    }
    names
}

/// 启动时解析好的字体（[`install`]）
///
/// **为什么缓存**：`installed()` 经 CoreText 枚举全部系统字体，单次数十毫秒；界面每帧要取十几次
/// （消息样式、代码块、行内代码、每个序号徽章……），实测不缓存时整窗重绘一帧约 620ms（S04 目检发现卡顿）。
/// 已安装字体在运行期间视为不变（v1 同样只在页面加载时解析 CSS 字体栈）。
struct ResolvedFonts {
    ui: Font,
    mono: Font,
}

impl Global for ResolvedFonts {}

/// 解析并缓存界面 / 等宽字体。由 [`crate::init_theme`] 调用，应用无需单独调用。
pub fn install(cx: &mut App) {
    let names = installed(cx);
    cx.set_global(ResolvedFonts {
        ui: resolve_stack(fonts::FONT_SANS, &names, SYSTEM_UI),
        mono: resolve_stack(fonts::FONT_MONO, &names, SYSTEM_MONO),
    });
}

/// 界面字体（`--font-sans`）
pub fn ui_font(cx: &App) -> Font {
    match cx.try_global::<ResolvedFonts>() {
        Some(f) => f.ui.clone(),
        None => resolve_stack(fonts::FONT_SANS, &installed(cx), SYSTEM_UI),
    }
}

/// 等宽字体（`--font-mono`）；栈中都没装时用 macOS 自带的 Menlo
pub fn mono_font(cx: &App) -> Font {
    match cx.try_global::<ResolvedFonts>() {
        Some(f) => f.mono.clone(),
        None => resolve_stack(fonts::FONT_MONO, &installed(cx), SYSTEM_MONO),
    }
}

/// 文字渲染模式：显式 `Grayscale`。
///
/// macOS 自 10.14 起不做次像素抗锯齿，灰度即其现状；显式设定让 Windows（默认次像素）
/// Windows 字体渲染与 macOS 的差异未验证。
pub const TEXT_RENDERING: TextRenderingMode = TextRenderingMode::Grayscale;

/// 应用文字渲染模式（启动时调用一次）
pub fn install_text_rendering(cx: &mut App) {
    cx.set_text_rendering_mode(TEXT_RENDERING);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn fallbacks(f: &Font) -> Vec<String> {
        f.fallbacks
            .as_ref()
            .map(|fb| fb.fallback_list().to_vec())
            .unwrap_or_default()
    }

    #[test]
    fn picks_first_installed_like_webkit() {
        // 本机情形：装了 Fira Code 与 PingFang SC
        let f = resolve_stack(
            fonts::FONT_SANS,
            &set(&["Fira Code", "PingFang SC"]),
            SYSTEM_UI,
        );
        assert_eq!(f.family.as_ref(), "Fira Code");
        // 其后：-apple-system → .SystemUIFont，再到 PingFang SC；未安装者与通用族被略去
        assert_eq!(
            fallbacks(&f),
            vec![SYSTEM_UI.to_string(), "PingFang SC".to_string()]
        );
    }

    #[test]
    fn falls_through_to_system_ui_when_fira_is_missing() {
        // 未装 Fira / JetBrains / Inter 的机器：WebKit 会落到 -apple-system
        let f = resolve_stack(fonts::FONT_SANS, &set(&["PingFang SC"]), SYSTEM_UI);
        assert_eq!(f.family.as_ref(), SYSTEM_UI);
        assert_eq!(fallbacks(&f), vec!["PingFang SC".to_string()]);
    }

    #[test]
    fn mono_uses_default_when_nothing_installed() {
        let f = resolve_stack(fonts::FONT_MONO, &set(&[]), SYSTEM_MONO);
        assert_eq!(f.family.as_ref(), SYSTEM_MONO);
        assert!(f.fallbacks.is_none());
    }
}
