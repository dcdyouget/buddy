use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const POLL: Duration = Duration::from_millis(25);
const HOTKEY: &str = "CmdOrCtrl+Alt+Shift+F19";

pub(crate) struct Fixture {
    pub(super) dir: PathBuf,
    pub(super) data_dir: PathBuf,
    pub(super) socket: PathBuf,
    pub(super) ready: PathBuf,
    pub(super) forwarded: PathBuf,
    pub(super) ipc_ack: PathBuf,
    pub(super) resume: PathBuf,
    pub(super) resume_ready: PathBuf,
    pub(super) hotkey_ack: PathBuf,
    pub(super) quit: PathBuf,
}

impl Fixture {
    pub(super) fn create() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("读取时间失败：{error}"))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("b11a-{}-{nonce:x}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|error| format!("创建专用目录失败：{error}"))?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("设置专用目录权限失败：{error}"))?;
        let data_dir = dir.join("data");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&data_dir)
            .map_err(|error| format!("创建专用数据目录失败：{error}"))?;
        fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("设置专用数据目录权限失败：{error}"))?;
        fs::write(
            data_dir.join("config.json"),
            format!(r#"{{"hotkey":"{HOTKEY}"}}"#),
        )
        .map_err(|error| format!("写专用热键配置失败：{error}"))?;
        fs::set_permissions(
            data_dir.join("config.json"),
            fs::Permissions::from_mode(0o600),
        )
        .map_err(|error| format!("设置专用配置权限失败：{error}"))?;
        Ok(Self {
            socket: dir.join("instance.sock"),
            ready: dir.join("ready"),
            forwarded: dir.join("forwarded"),
            ipc_ack: dir.join("ipc-ack"),
            resume: dir.join("resume"),
            resume_ready: dir.join("resume-ready"),
            hotkey_ack: dir.join("hotkey-ack"),
            quit: dir.join("quit"),
            data_dir,
            dir,
        })
    }

    pub(super) fn lock(&self) -> PathBuf {
        self.socket.with_extension("lock")
    }

    pub(super) fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub(crate) struct OwnedChild {
    child: Option<Child>,
}

impl OwnedChild {
    pub(super) fn spawn_owner(fixture: &Fixture) -> Result<Self, String> {
        let executable =
            std::env::current_exe().map_err(|error| format!("读取应用探针路径失败：{error}"))?;
        let child = Command::new(executable)
            .args([
                "--child".to_owned(),
                fixture.socket.display().to_string(),
                fixture.data_dir.display().to_string(),
                fixture.ready.display().to_string(),
                fixture.ipc_ack.display().to_string(),
                fixture.resume.display().to_string(),
                fixture.resume_ready.display().to_string(),
                fixture.hotkey_ack.display().to_string(),
                fixture.quit.display().to_string(),
            ])
            .spawn()
            .map_err(|error| format!("启动应用 child 失败：{error}"))?;
        Ok(Self { child: Some(child) })
    }

    pub(super) fn spawn_secondary(socket: &Path, forwarded: &Path) -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("读取 secondary 探针路径失败：{error}"))?;
        let child = Command::new(executable)
            .args([
                "--secondary".to_owned(),
                socket.display().to_string(),
                forwarded.display().to_string(),
            ])
            .spawn()
            .map_err(|error| format!("启动 secondary child 失败：{error}"))?;
        Ok(Self { child: Some(child) })
    }

    pub(super) fn spawn_f19_sender() -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("读取 F19 sender 路径失败：{error}"))?;
        let child = Command::new(executable)
            .arg("--send-f19")
            .spawn()
            .map_err(|error| format!("启动独立 F19 sender 失败：{error}"))?;
        Ok(Self { child: Some(child) })
    }

    pub(super) fn wait_ready(&mut self, marker: &Path) -> Result<(), String> {
        let started = Instant::now();
        while started.elapsed() < TIMEOUT {
            if marker.is_file() {
                return Ok(());
            }
            let status = self
                .child
                .as_mut()
                .ok_or_else(|| "应用 child 已结束".to_string())?
                .try_wait()
                .map_err(|error| format!("检查应用 child 失败：{error}"))?;
            if let Some(status) = status {
                return Err(format!("应用 child 在 READY 前退出：{status}"));
            }
            thread::sleep(POLL);
        }
        Err("等待应用 child READY 超时".into())
    }

    pub(super) fn wait_success(&mut self) -> Result<(), String> {
        let started = Instant::now();
        while started.elapsed() < TIMEOUT {
            let status = self
                .child
                .as_mut()
                .ok_or_else(|| "应用 child 已结束".to_string())?
                .try_wait()
                .map_err(|error| format!("等待应用 child 失败：{error}"))?;
            if let Some(status) = status {
                self.child = None;
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("应用 child 退出失败：{status}"))
                };
            }
            thread::sleep(POLL);
        }
        Err("应用 child 正常退出超时".into())
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        self.child = None;
    }
}

pub(super) fn wait_marker_blocking(
    child: &mut OwnedChild,
    marker: &Path,
    label: &str,
) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < TIMEOUT {
        if marker.is_file() {
            return Ok(());
        }
        let status = child
            .child
            .as_mut()
            .ok_or_else(|| format!("{label} 前 owner 已结束"))?
            .try_wait()
            .map_err(|error| format!("等待 {label} 时检查 owner 失败：{error}"))?;
        if let Some(status) = status {
            return Err(format!("{label} 前 owner 退出：{status}"));
        }
        thread::sleep(POLL);
    }
    Err(format!("等待 {label} 超时"))
}
