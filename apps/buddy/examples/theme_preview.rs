//! Phase 03 主题预览：色板 / 排版 / 圆角 / 阴影，浅深切换。
//!
//! ```text
//! cargo run -p buddy-app --example theme_preview               # 打开预览窗口（目检，对照 v1）
//! cargo run -p buddy-app --example theme_preview -- --selftest  # 在真实 App 中自检安装 / 切换 / 字体 / 渲染模式后退出
//! ```

use buddy_ui::gpui::{
    App, Bounds, Context, FontWeight, Hsla, Render, Rgba, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, relative, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::theme_system::{
    Appearance, BuddyTheme, Theme, box_shadows, fonts, set_appearance,
    tokens::metrics as m, typography::{self, TextRole},
};

struct Preview;

fn hsla(c: Rgba) -> Hsla {
    Hsla::from(c)
}

fn role_sample(role: TextRole, label: &str, cx: &App) -> impl IntoElement {
    let t = cx.buddy_theme();
    div()
        .font(fonts::ui_font(cx))
        .text_size(px(role.size))
        .font_weight(FontWeight(role.weight))
        .line_height(relative(role.line_height))
        .text_color(hsla(t.colors.text_primary))
        .child(format!("{label} {}px/{} — Buddy 让你随时唤起 AI 对话 0123", role.size, role.weight))
}

impl Render for Preview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = *cx.buddy_theme();
        let c = t.colors;
        let muted = hsla(c.text_muted);
        let ui_font = fonts::ui_font(cx);
        let mono = fonts::mono_font(cx);

        let swatches = c.entries().into_iter().map(|(name, color)| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .w(px(300.0))
                .child(
                    div()
                        .size(px(22.0))
                        .rounded(px(m::RADIUS_SM))
                        .border_1()
                        .border_color(hsla(c.border_default))
                        .bg(hsla(color)),
                )
                .child(div().text_size(px(m::FONT_SIZE_SM)).text_color(muted).child(name))
        });

        let radii = [m::RADIUS_SM, m::RADIUS_MD, m::RADIUS_LG, m::RADIUS_XL, m::RADIUS_FULL].map(|r| {
            div()
                .size(px(48.0))
                .rounded(px(r)) // GPUI 会把过大的圆角限制在短边一半，9999 即胶囊形
                .bg(hsla(c.buddy_primary))
                .text_color(hsla(c.text_on_primary))
                .text_size(px(m::FONT_SIZE_XS))
                .flex()
                .items_center()
                .justify_center()
                .child(format!("{r}"))
        });

        let s = t.shadows;
        let shadows = [
            ("static", s.shadow_static),
            ("floating-sm", s.shadow_floating_sm),
            ("floating-md", s.shadow_floating_md),
            ("composer", s.shadow_composer),
        ]
        .map(|(name, layers)| {
            div()
                .w(px(120.0))
                .h(px(64.0))
                .rounded(px(m::RADIUS_LG))
                .bg(hsla(c.bg_elevated))
                .shadow(box_shadows(layers))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(m::FONT_SIZE_SM))
                .text_color(muted)
                .child(name)
        });

        let toggle = div()
            .id("toggle")
            .px_3()
            .py_1()
            .rounded(px(m::RADIUS_MD))
            .bg(hsla(c.buddy_primary))
            .text_color(hsla(c.text_on_primary))
            .cursor_pointer()
            .child(match t.appearance {
                Appearance::Light => "切换到深色",
                Appearance::Dark => "切换到浅色",
            })
            .on_click(|_, _, cx| {
                let next = match cx.buddy_theme().appearance {
                    Appearance::Light => Appearance::Dark,
                    Appearance::Dark => Appearance::Light,
                };
                set_appearance(next, cx);
            });

        div()
            .id("root")
            .size_full()
            .overflow_y_scroll()
            .bg(hsla(c.bg_canvas))
            .font(ui_font.clone())
            .text_color(hsla(c.text_primary))
            .p(px(m::SPACE_6))
            .flex()
            .flex_col()
            .gap(px(m::SPACE_5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(toggle)
                    .child(div().text_color(muted).child(format!(
                        "外观：{:?}　界面字体：{}（回退 {:?}）　等宽：{}",
                        t.appearance,
                        ui_font.family,
                        ui_font.fallbacks.as_ref().map(|f| f.fallback_list().to_vec()).unwrap_or_default(),
                        mono.family
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p(px(m::SPACE_4))
                    .rounded(px(m::RADIUS_XL))
                    .bg(hsla(c.bg_surface))
                    .child(role_sample(typography::TITLE, "title", cx))
                    .child(role_sample(typography::H3, "h3", cx))
                    .child(role_sample(typography::BODY, "body", cx))
                    .child(role_sample(typography::BODY_SM, "body-sm", cx))
                    .child(role_sample(typography::CAPTION, "caption", cx))
                    .child(
                        div()
                            .font(mono)
                            .text_size(px(m::FONT_SIZE_BASE))
                            .p(px(m::SPACE_3))
                            .rounded(px(m::RADIUS_MD))
                            .bg(hsla(c.code_bg))
                            .text_color(hsla(c.code_text))
                            .child("fn main() { println!(\"等宽 mono 0O1lI\"); }"),
                    ),
            )
            .child(div().flex().gap(px(m::SPACE_4)).children(radii))
            .child(div().flex().gap(px(m::SPACE_6)).p(px(m::SPACE_4)).children(shadows))
            .child(div().flex().flex_wrap().gap_2().children(swatches))
    }
}

/// 在真实 App 中验证：安装 / 读回 / 切换、文字渲染模式、本机字体解析
fn selftest(cx: &mut App) -> bool {
    let mut ok = true;
    let mut check = |name: &str, pass: bool| {
        println!("{} {name}", if pass { "PASS" } else { "FAIL" });
        ok &= pass;
    };

    Theme::install(Appearance::Light, cx);
    check("S03-01 安装浅色后读回 Light", cx.buddy_theme().appearance == Appearance::Light);
    let light_canvas = cx.buddy_theme().colors.bg_canvas;
    set_appearance(Appearance::Dark, cx);
    check("S03-06 切换后读回 Dark", cx.buddy_theme().appearance == Appearance::Dark);
    check("S03-06 切换后颜色随之变化", cx.buddy_theme().colors.bg_canvas != light_canvas);
    set_appearance(Appearance::Light, cx);
    check("S03-06 切回后与首次一致", cx.buddy_theme().colors.bg_canvas == light_canvas);

    fonts::install_text_rendering(cx);
    check(
        "S03-05 text_rendering_mode 读回 Grayscale",
        cx.text_rendering_mode() == fonts::TEXT_RENDERING,
    );

    let names = cx.text_system().all_font_names();
    let has = |n: &str| names.iter().any(|x| x == n);
    let ui = fonts::ui_font(cx);
    let mono = fonts::mono_font(cx);
    println!(
        "fonts: installed(Fira Code)={} installed(PingFang SC)={} installed(JetBrains Mono)={} installed(Inter)={}",
        has("Fira Code"), has("PingFang SC"), has("JetBrains Mono"), has("Inter")
    );
    println!(
        "fonts: ui={} fallbacks={:?} mono={} mono_fallbacks={:?}",
        ui.family,
        ui.fallbacks.as_ref().map(|f| f.fallback_list().to_vec()),
        mono.family,
        mono.fallbacks.as_ref().map(|f| f.fallback_list().to_vec())
    );
    // 与 WebKit 规则一致：栈中第一个已安装者
    let expected_ui = if has("Fira Code") { "Fira Code" } else if has("JetBrains Mono") { "JetBrains Mono" } else if has("Inter") { "Inter" } else { ".SystemUIFont" };
    check("S03-05 界面字体 = 栈中第一个已安装者", ui.family.as_ref() == expected_ui);
    // 解析失败时 GPUI 会退到全局回退栈首项 `.ZedMono`；得到不同的 FontId 即说明首选字体被真正解析
    let resolved = cx.text_system().resolve_font(&ui);
    let zed_fallback = cx.text_system().resolve_font(&buddy_ui::gpui::font(".ZedMono"));
    check("S03-05 界面字体被真正解析（未落到 GPUI 全局回退）", resolved != zed_fallback);
    ok
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let self_test = std::env::args().any(|a| a == "--selftest");
    application().run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        if self_test {
            let ok = selftest(cx);
            println!("RESULT: {}", if ok { "PASS" } else { "FAIL" });
            std::process::exit(if ok { 0 } else { 1 });
        }
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        let bounds = Bounds::centered(None, size(px(1000.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |_, cx| cx.new(|_| Preview),
        )
        .expect("open_window 失败");
        cx.activate(true);
    });
}
