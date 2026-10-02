//! S07-01 / S07-02 主窗口外壳预览：真实工厂、沙盒 engine 与本地 mock 模型。
//!
//! `cargo run -p buddy-app --example shell_preview`；`--dark` 深色；
//! `--selftest` 验证窗口与行为；`--selftest-window` 仅验证尺寸和原生属性。
//! `--selftest-behavior` 验证系统输入，需已解锁桌面。Cmd+Q 退出手动预览。

use buddy_ui::gpui::{App, AsyncApp};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::{self, config::ShellConfig};

#[path = "shell_preview/external_target.rs"]
mod external_target;
#[path = "shell_preview/fixture.rs"]
mod fixture;
#[path = "shell_preview/selftest.rs"]
mod selftest;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--external-target") {
        external_target::run_child();
    }
    let self_test = args.iter().any(|arg| arg == "--selftest");
    let behavior_test = args.iter().any(|arg| arg == "--selftest-behavior");
    let window_test = args.iter().any(|arg| arg == "--selftest-window");
    let dark = args.iter().any(|arg| arg == "--dark");
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            shell::init(cx);
            if self_test || behavior_test || window_test {
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let passed = if behavior_test {
                        selftest::run_behaviors(cx).await
                    } else if window_test {
                        selftest::run_windows(cx).await
                    } else {
                        selftest::run(cx).await
                    };
                    std::process::exit(if passed { 0 } else { 1 });
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
                        Ok(handle) => {
                            if let Err(error) = shell::runtime::install(handle, cx).await {
                                log::error!("安装预览窗口事件失败：{error}");
                            }
                            if let Err(error) = shell::runtime::show(handle, cx).await {
                                log::error!("显示预览窗口失败：{error}");
                            }
                        }
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
