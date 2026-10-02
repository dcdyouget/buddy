//! S07-01 / S07-02 主窗口外壳预览：真实工厂、沙盒 engine 与本地 mock 模型。
//!
//! `cargo run -p buddy-app --example shell_preview`；`--dark` 深色；
//! `--selftest` 验证真实主窗口尺寸与 macOS 原生属性。Cmd+Q 退出手动预览。

use buddy_ui::gpui::{App, AsyncApp};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::{self, config::ShellConfig};

#[path = "shell_preview/fixture.rs"]
mod fixture;
#[path = "shell_preview/selftest.rs"]
mod selftest;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    let self_test = args.iter().any(|arg| arg == "--selftest");
    let dark = args.iter().any(|arg| arg == "--dark");
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            shell::init(cx);
            if self_test {
                cx.spawn(async move |cx: &mut AsyncApp| {
                    std::process::exit(if selftest::run(cx).await { 0 } else { 1 });
                })
                .detach();
            } else {
                cx.observe_keystrokes(|event, _, cx| {
                    if event.keystroke.unparse() == "cmd-q" {
                        cx.quit();
                    }
                })
                .detach();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let engine = fixture::manual_engine(dark);
                    match shell::open_main_window(engine, ShellConfig::default(), cx).await {
                        Ok(_) => cx.update(|cx| cx.activate(true)),
                        Err(error) => {
                            log::error!("创建外壳预览失败：{error}");
                            cx.update(|cx| cx.quit());
                        }
                    }
                })
                .detach();
            }
        });
}
