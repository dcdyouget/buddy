//! Buddy 可执行入口
//!
//! 职责：组装 `buddy-ui`（GPL）与 `buddy-engine`（MIT），启动 GPUI 应用。
//!
//! **S01-01 阶段为骨架**：只验证 workspace 能编译、依赖方向正确、
//! 主题初始化顺序可用。窗口外壳归 `S07-*`，页面归 `S05-*` / `S06-*`。

// 全部 GPUI 表面从 `buddy_ui` 取 —— app 不直接依赖 zed 侧 crate
use buddy_ui::gpui::{
    App, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::ActiveTheme;

struct Root;

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.colors().element_background)
            .text_color(theme.colors().text)
            .p_4()
            .gap_2()
            .child("Buddy v2.0.0-gpui — workspace 骨架")
            .child(format!("engine VERSION = {}", buddy_engine::VERSION))
            .child("（S01-01 阶段：仅验证分层与依赖方向）")
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    application().run(|cx: &mut App| {
        // ⚠️ 顺序不可颠倒：theme::init → set_theme_settings_provider
        //    （漏第二步会在渲染 ui 组件时 panic，见 buddy_ui 的文档）
        buddy_ui::init_theme(cx);

        let bounds = Bounds::centered(None, size(px(560.0), px(200.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Root),
        )
        .expect("open_window 失败");

        cx.activate(true);
    });
}
