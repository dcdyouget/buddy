use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use buddy_ui::gpui::{AsyncApp, WindowHandle};
use buddy_ui::shell::AppShell;
use buddy_ui::shell::autostart::{AutostartBackend, SystemAutostart};

use super::super::protocol::{Fixture, POLL, STATE_TIMEOUT};

/// Delegates to the real LaunchAgent while recording set targets for the
/// forced config-write failure stage.
pub(super) struct ObservedAutostart {
    service: Rc<SystemAutostart>,
    set_targets: RefCell<Vec<bool>>,
}

impl ObservedAutostart {
    pub(super) fn new(service: Rc<SystemAutostart>) -> Self {
        Self {
            service,
            set_targets: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn clear_set_targets(&self) {
        self.set_targets.borrow_mut().clear();
    }

    pub(super) fn set_targets(&self) -> Vec<bool> {
        self.set_targets.borrow().clone()
    }
}

impl AutostartBackend for ObservedAutostart {
    fn query(&self) -> Result<bool, String> {
        self.service.query()
    }

    fn set(&self, enabled: bool) -> Result<(), String> {
        self.set_targets.borrow_mut().push(enabled);
        self.service.set(enabled)
    }
}

/// Replaces config.json with a directory so the real atomic rename fails.
/// Drop restores the original bytes if the phase exits unexpectedly.
pub(super) struct ConfigPathGuard {
    path: PathBuf,
    original: Vec<u8>,
    restored: bool,
}

impl ConfigPathGuard {
    pub(super) fn block(path: PathBuf) -> Result<Self, String> {
        let original = fs::read(&path).map_err(|error| format!("读取配置样本失败：{error}"))?;
        fs::remove_file(&path).map_err(|error| format!("移除配置样本失败：{error}"))?;
        if let Err(error) = fs::create_dir(&path) {
            let _ = fs::write(&path, &original);
            return Err(format!("创建配置故障目录失败：{error}"));
        }
        Ok(Self {
            path,
            original,
            restored: false,
        })
    }

    pub(super) fn restore(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        if let Err(error) = fs::remove_dir(&self.path) {
            errors.push(format!("删除配置故障目录失败：{error}"));
        }
        if let Err(error) = fs::write(&self.path, &self.original) {
            errors.push(format!("恢复配置文件失败：{error}"));
        } else if fs::read(&self.path).ok().as_deref() != Some(self.original.as_slice()) {
            errors.push("恢复后的配置样本字节不一致".into());
        }
        if errors.is_empty() {
            self.restored = true;
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }
}

impl Drop for ConfigPathGuard {
    fn drop(&mut self) {
        if !self.restored {
            if let Err(error) = self.restore() {
                eprintln!("FAIL S07-09/13 config cleanup：{error}");
            }
        }
    }
}

pub(super) async fn wait_autostart_save_failure(
    service: &SystemAutostart,
    observed: &ObservedAutostart,
    fixture: &Fixture,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let plist = fixture.plist()?;
    let deadline = Instant::now() + STATE_TIMEOUT;
    loop {
        let targets = observed.set_targets();
        let entry = service.query()?;
        if targets == [false, true] && entry && plist.exists() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "写盘失败回滚读回异常：set_targets={targets:?} launch_agent_entry={entry} plist_exists={}",
                plist.exists()
            ));
        }
        cx.background_executor().timer(POLL).await;
    }
}

pub(super) fn router_autostart(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    handle
        .read_with(cx, |shell, app| {
            shell.router().read(app).config().auto_start
        })
        .map_err(|error| format!("读取 Router 自启配置失败：{error}"))
}
