//! S07-11 真实 GPUI 应用退出、IPC 唤回与热键重绑探针。
//!
//! child 走产品的 shell::init、主窗口、runtime 和 lifecycle 接线，使用自己
//! 的数据目录与 F19 热键；成功路径只调用 cx.quit()，不使用
//! process::exit(0) 绕过 on_app_quit。直接调用 `resume_after_wake` 仅模拟恢复，
//! 不声称覆盖真实系统睡眠 / 唤醒。

#[cfg(target_os = "macos")]
#[path = "lifecycle_app_probe/probe.rs"]
mod macos_probe;

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("[S07-11] FAIL：真实 GPUI 生命周期探针仅在 macOS 运行；Windows 归 Phase 09");
        std::process::exit(1);
    }

    #[cfg(target_os = "macos")]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let result = if args.is_empty() {
            macos_probe::run()
        } else {
            macos_probe::run_child(&args)
        };
        if let Err(error) = result {
            if error.starts_with("BLOCKED") {
                eprintln!("[S07-11] {error}");
                std::process::exit(2);
            }
            eprintln!("[S07-11] FAIL：{error}");
            std::process::exit(1);
        }
    }
}
