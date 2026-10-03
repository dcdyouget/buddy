//! S07-09 / S07-13 真实系统托盘菜单探针。
//!
//! 默认模式是 supervisor：启动一个真实 GPUI child，持有专用 LaunchAgent 的清理句柄，
//! 等待主 agent 在桌面上完成真实左键、右键和菜单项点击。动作不能由此探针直接发送
//! `MenuAction` 替代；每个阶段都用 `*.ready` / `*.continue` / `*.ack` 文件握手，
//! child 只在真实状态读回后写 ack。
//!
//! ```text
//! cargo run -q -p buddy-app --example tray_os_probe
//! ```
//!
//! 启动后按终端打印的路径操作：看到 `show.ready` 后实际左键点击菜单栏 Buddy 图标，
//! 再执行对应的 `touch show.continue`；后续 settings / autostart 阶段同理。最后看到
//! `quit.ready` 后实际右键点击“退出”。supervisor 会在 child 成功、失败或超时后清理
//! 唯一 LaunchAgent、sandbox、marker 和 child。

use std::process::ExitCode;

#[cfg(target_os = "macos")]
#[path = "tray_os_probe/child.rs"]
mod child;
#[cfg(target_os = "macos")]
#[path = "tray_os_probe/protocol.rs"]
mod protocol;
#[cfg(target_os = "macos")]
#[path = "tray_os_probe/supervisor.rs"]
mod supervisor;

fn main() -> ExitCode {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("BLOCKED S07-09/13：真实 tray OS 探针仅支持 macOS，Windows 归 Phase 09");
        return ExitCode::FAILURE;
    }

    #[cfg(target_os = "macos")]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let result = if args.first().map(String::as_str) == Some("--child") {
            child::run(&args)
        } else {
            supervisor::run()
        };
        if let Err(error) = result {
            eprintln!("FAIL S07-09/13：{error}");
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    }
}
