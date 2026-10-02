//! Buddy 产品入口：装配引擎、真实页面与统一主窗口外壳。
use buddy_engine::{chat::ChatEngine, storage};
use buddy_ui::gpui::{App, AsyncApp};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::{self, config::ShellConfig};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    // 与 v1 相同的应用数据目录；不进行历史迁移。
    let data_dir = match storage::default_data_dir() {
        Ok(path) => path,
        Err(error) => {
            log::error!("无法定位应用数据目录：{error}");
            std::process::exit(1);
        }
    };
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            shell::init(cx);
            cx.spawn(async move |cx: &mut AsyncApp| {
                if let Err(error) =
                    shell::open_main_window(ChatEngine::new(data_dir), ShellConfig::default(), cx)
                        .await
                {
                    log::error!("创建主窗口失败：{error}");
                    cx.update(|cx| cx.quit());
                }
            })
            .detach();
        });
}
