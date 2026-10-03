//! S07-10 macOS LaunchAgent 独立探针。
//!
//! 使用唯一名称 `BuddyS0710Probe-<pid>`，不会操作正式 Buddy 登录项。

use buddy_ui::shell::autostart::SystemAutostart;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    if let Err(error) = run() {
        eprintln!("[S07-10] FAIL：{error}");
        std::process::exit(1);
    }
    println!("[S07-10] PASS：LaunchAgent 查询、开启、读回、关闭和清理均完成");
}

fn run() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("S07-10 独立探针仅在 macOS 运行；Windows 归 Phase 09".into());
    }

    let path = std::env::current_exe().map_err(|error| format!("读取探针路径失败：{error}"))?;
    let app_name = format!("BuddyS0710Probe-{}", std::process::id());
    let plist = launch_agent_path(&app_name)?;
    if plist.exists() {
        return Err(format!(
            "专用探针 plist 已存在，拒绝覆盖：{}",
            plist.display()
        ));
    }

    let service = SystemAutostart::with_identity(&app_name, &path)?;
    let before = service.query()?;
    if before {
        return Err("专用探针初始状态已开启，拒绝改变未知状态".into());
    }

    let operation = (|| {
        println!("[S07-10] app_name={app_name}");
        println!("[S07-10] app_path={}", path.display());
        println!("[S07-10] before={before}");
        service.set(true)?;
        if !service.query()? {
            return Err("开启后系统读回仍为关闭".into());
        }
        if !plist.exists() {
            return Err("开启后 LaunchAgent plist 未落盘".into());
        }
        let arguments = plist_program_arguments(&plist)?;
        let executable = path.to_string_lossy();
        if !arguments
            .iter()
            .any(|argument| argument.as_str() == Some(executable.as_ref()))
        {
            return Err("LaunchAgent 的 ProgramArguments 未包含探针可执行文件路径".into());
        }
        println!("[S07-10] enabled=true plist={} path=true", plist.display());
        service.set(false)?;
        if service.query()? {
            return Err("关闭后系统读回仍为开启".into());
        }
        if plist.exists() {
            return Err("关闭后专用 LaunchAgent plist 未清理".into());
        }
        println!("[S07-10] disabled=false plist_removed=true");
        Ok(())
    })();

    let cleanup = service.set(false);
    match (operation, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("探针完成但最终清理失败：{error}")),
        (Err(error), Err(cleanup)) => Err(format!("{error}；最终清理失败：{cleanup}")),
    }
}

fn plist_program_arguments(plist: &std::path::Path) -> Result<Vec<serde_json::Value>, String> {
    let output = Command::new("plutil")
        .args(["-extract", "ProgramArguments", "json", "-o", "-"])
        .arg(plist)
        .output()
        .map_err(|error| format!("调用 plutil 读取 ProgramArguments 失败：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "plutil 读取 ProgramArguments 失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("解析 ProgramArguments JSON 失败：{error}"))?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "LaunchAgent 的 ProgramArguments 不是数组".into())
}

fn launch_agent_path(app_name: &str) -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "无法读取 HOME，不能定位 LaunchAgent plist".to_string())?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{app_name}.plist")))
}
