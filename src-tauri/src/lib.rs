// 应用入口模块：窗口创建、快捷键注册、系统托盘、毛玻璃效果及圆角等核心初始化逻辑

use parking_lot::Mutex;
use tauri::Manager;
use tauri_plugin_global_shortcut::Shortcut;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

mod commands;
mod hotkey;
mod mcp;
mod models;
mod platform;
mod providers;
mod storage;
mod streaming;
mod tools;
mod tray;
mod window;

// Re-export key types for commands and hotkey modules
pub use commands::{CancelState, QuestionState};
pub use window::positioning::SavedWindowPositions;

/// 应用主入口：配置并启动整个 Tauri 应用
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_targets = if cfg!(debug_assertions) {
        vec![
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some("buddy".to_string()),
            }),
        ]
    } else {
        vec![Target::new(TargetKind::LogDir {
            file_name: Some("buddy".to_string()),
        })]
    };
    let log_level = if cfg!(debug_assertions) {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Warn
    };
    let (max_log_file_size, log_rotation) = if cfg!(debug_assertions) {
        (5 * 1024 * 1024, RotationStrategy::KeepSome(3))
    } else {
        (2 * 1024 * 1024, RotationStrategy::KeepSome(2))
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(log_targets)
                .level(log_level)
                .max_file_size(max_log_file_size)
                .rotation_strategy(log_rotation)
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(CancelState {
            sender: Mutex::new(None),
        })
        .manage(commands::ApprovalState {
            pending: Mutex::new(Vec::new()),
            approve_all_for_turn: std::sync::atomic::AtomicBool::new(false),
        })
        .manage(commands::QuestionState {
            pending: Mutex::new(Vec::new()),
        })
        .manage(hotkey::HotkeyState {
            current: Mutex::new(None),
        })
        .setup(|app| {
            let log_dir = app
                .path()
                .app_log_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("无法解析：{error}"));
            log::info!(
                "[app] Buddy 启动: version={}, log_dir={}",
                app.package_info().version,
                log_dir
            );
            match storage::migrate_legacy_image_attachments(app.handle()) {
                Ok(count) if count > 0 => {
                    log::info!("已将 {} 张旧版 Base64 图片迁移为本地附件", count);
                }
                Ok(_) => {}
                Err(error) => log::warn!("迁移旧版图片附件失败：{}", error),
            }
            // 加载配置
            let config =
                storage::get_config(app.handle()).unwrap_or_else(|_| models::AppConfig::default());
            let shortcut: Shortcut = config.hotkey.parse().unwrap_or(Shortcut::new(
                None,
                tauri_plugin_global_shortcut::Code::KeyJ,
            ));

            app.state::<hotkey::HotkeyState>()
                .current
                .lock()
                .replace(shortcut);
            if let Err(error) = hotkey::register(app.handle(), shortcut) {
                log::warn!("[hotkey] 启动时注册全局快捷键失败: {}", error);
            }

            // 创建系统托盘
            tray::create_tray(app)?;

            // macOS 毛玻璃效果和圆角
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                platform::macos::apply_macos_window_effects(&window);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_message,
            commands::stop_generation,
            commands::approve_tool_call,
            commands::answer_tool_question,
            commands::get_config,
            commands::save_config,
            commands::fetch_models,
            commands::test_latency,
            commands::load_messages,
            commands::get_message_count,
            commands::save_message,
            commands::save_chat_image,
            commands::delete_chat_image,
            commands::download_generated_image,
        ])
        .on_window_event(|window, event| {
            window::events::setup_window_event_handler(&window, &event);
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
