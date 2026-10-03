use std::fs;
use std::rc::Rc;

use buddy_ui::shell::autostart::SystemAutostart;

use super::protocol::{Fixture, OwnedChild, cleanup_autostart, remove_fixture};

fn validate_protocol(fixture: &Fixture) -> Result<(), String> {
    if fixture.marker("fail").exists() {
        let detail = fs::read_to_string(fixture.marker("fail"))
            .unwrap_or_else(|error| format!("无法读取 fail marker：{error}"));
        return Err(format!("child 写入 fail marker：{}", detail.trim()));
    }
    let expected = [
        ("show.ack", "visible=true key=true active=true\n"),
        ("settings.ack", "page=settings active=true\n"),
        (
            "autostart-enable.ack",
            "launch_agent_entry=true disk=true plist=true\n",
        ),
        (
            "autostart-save-failure.ack",
            "config_write=failed set_targets=false,true launch_agent_entry=true disk=true router=true plist=true bytes_equal=true\n",
        ),
        (
            "autostart-disable.ack",
            "launch_agent_entry=false disk=false plist=false\n",
        ),
    ];
    for (name, expected_content) in expected {
        let path = fixture.marker(name);
        let actual =
            fs::read_to_string(&path).map_err(|error| format!("缺少或无法读取 {name}：{error}"))?;
        if actual != expected_content {
            return Err(format!(
                "{name} 读回错误：expected={expected_content:?} actual={actual:?}"
            ));
        }
    }
    if !fixture.marker("quit.ready").is_file() {
        return Err("缺少 quit.ready，未证明真实退出菜单阶段已就绪".into());
    }
    Ok(())
}

pub(crate) fn run() -> Result<(), String> {
    let fixture = Fixture::create()?;
    let executable = std::env::current_exe()
        .map_err(|error| format!("读取 supervisor 可执行文件失败：{error}"))?;
    let service = Rc::new(SystemAutostart::with_identity(
        &fixture.app_name,
        &executable,
    )?);
    let plist = fixture.plist()?;
    if plist.exists() {
        return Err(format!(
            "专用 LaunchAgent 已存在，拒绝覆盖：{}",
            plist.display()
        ));
    }
    if service.query()? {
        return Err("专用 LaunchAgent entry 初始已存在，拒绝改变未知状态".into());
    }

    let mut child = OwnedChild::spawn(&fixture)?;
    fixture.print_instructions();
    if let Err(error) = child.wait_ready(&fixture.marker("ready")) {
        let reap = child.terminate_and_wait();
        let cleanup = cleanup_autostart(&service, &plist);
        return Err(format!("{error}；回收={reap:?}；清理={cleanup:?}"));
    }
    println!("[S07-09/13] READY：可以开始真实桌面点击");
    let status = child.wait_exit();
    let reap = if status.is_err() {
        child.terminate_and_wait()
    } else {
        Ok(())
    };
    let protocol = validate_protocol(&fixture);
    let cleanup = cleanup_autostart(&service, &plist);
    let removed = remove_fixture(&fixture);
    if status
        .as_ref()
        .map(|status| status.success())
        .unwrap_or(false)
        && protocol.is_ok()
        && cleanup.is_ok()
        && removed.is_ok()
    {
        println!("PASS S07-09/13：真实菜单链路、专用 LaunchAgent 和 sandbox 已清理");
        return Ok(());
    }
    if status.is_err() || reap.is_err() || protocol.is_err() || cleanup.is_err() || removed.is_err()
    {
        return Err(format!(
            "真实 tray 探针失败：status={status:?} reap={reap:?} protocol={protocol:?} cleanup={cleanup:?} sandbox={removed:?}"
        ));
    }
    Err(format!("真实 tray child 返回失败：status={status:?}"))
}
