#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 初始化日志：debug 模式下默认 info 级别，release 模式下默认 warn
    // 仍可通过 RUST_LOG 环境变量覆盖
    let mut builder = env_logger::Builder::new();
    if cfg!(debug_assertions) {
        builder.filter_level(log::LevelFilter::Info);
    } else {
        builder.filter_level(log::LevelFilter::Warn);
    }
    builder.parse_default_env(); // 允许 RUST_LOG 覆盖
    builder.init();

    buddy_lib::run();
}
