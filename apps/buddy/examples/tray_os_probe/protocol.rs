use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use buddy_ui::shell::autostart::SystemAutostart;

pub(crate) const POLL: Duration = Duration::from_millis(40);
pub(crate) const STATE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const PHASE_TIMEOUT: Duration = Duration::from_secs(600);

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) app_name: String,
    cleanup_on_drop: bool,
}

impl Fixture {
    pub(crate) fn create() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("读取时间失败：{error}"))?
            .as_nanos();
        let pid = std::process::id();
        let root = std::env::temp_dir().join(format!("buddy-s0713-tray-{pid}-{nonce:x}"));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|error| format!("创建托盘探针目录失败：{error}"))?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("设置托盘探针目录权限失败：{error}"))?;
        let data = root.join("data");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&data)
            .map_err(|error| format!("创建托盘 engine 沙盒失败：{error}"))?;
        fs::set_permissions(&data, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("设置托盘 engine 沙盒权限失败：{error}"))?;
        let app_name = format!("BuddyS0713Tray-{pid}");
        Ok(Self {
            root,
            data,
            app_name,
            cleanup_on_drop: true,
        })
    }

    pub(crate) fn marker(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub(crate) fn plist(&self) -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "无法读取 HOME，不能定位专用 LaunchAgent".to_string())?;
        Ok(home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", self.app_name)))
    }

    pub(crate) fn print_instructions(&self) {
        println!(
            "[S07-09/13] fixture={} app_name={} data={} plist={}",
            self.root.display(),
            self.app_name,
            self.data.display(),
            self.plist()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("ERROR:{error}")),
        );
        println!("[S07-09/13] 每阶段必须真实点击后 touch 对应 continue；不要直接发送 MenuAction。");
        println!(
            "[S07-09/13] 自启 ack 的 launch_agent_entry 表示 LaunchAgent entry/plist 读回，不代表 launchd 已加载。"
        );
        for phase in ["show", "settings", "autostart-enable", "autostart-disable"] {
            println!(
                "[S07-09/13] 等待 {}.ready 后，实际点击，再执行 touch {}",
                phase,
                self.marker(&format!("{phase}.continue")).display()
            );
        }
        println!("[S07-09/13] 等待 quit.ready 后实际点击‘退出’；child 退出后 supervisor 自动清理");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

pub(crate) struct OwnedChild {
    child: Option<Child>,
}

impl OwnedChild {
    pub(crate) fn spawn(fixture: &Fixture) -> Result<Self, String> {
        let executable =
            std::env::current_exe().map_err(|error| format!("读取托盘 child 路径失败：{error}"))?;
        let child = Command::new(executable)
            .args([
                "--child".to_string(),
                fixture.root.display().to_string(),
                fixture.app_name.clone(),
            ])
            .spawn()
            .map_err(|error| format!("启动托盘 child 失败：{error}"))?;
        Ok(Self { child: Some(child) })
    }

    pub(crate) fn wait_ready(&mut self, marker: &Path) -> Result<(), String> {
        let deadline = Instant::now() + PHASE_TIMEOUT;
        loop {
            if marker.is_file() {
                return Ok(());
            }
            let child = self
                .child
                .as_mut()
                .ok_or_else(|| "托盘 child 已结束".to_string())?;
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("检查托盘 child 失败：{error}"))?
            {
                return Err(format!("托盘 child 在 ready 前退出：{status}"));
            }
            if Instant::now() >= deadline {
                return Err(format!("等待 {} 超时", marker.display()));
            }
            thread::sleep(POLL);
        }
    }

    pub(crate) fn wait_exit(&mut self) -> Result<std::process::ExitStatus, String> {
        let deadline = Instant::now() + PHASE_TIMEOUT;
        loop {
            let child = self
                .child
                .as_mut()
                .ok_or_else(|| "托盘 child 已结束且无退出状态".to_string())?;
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("等待托盘 child 失败：{error}"))?
            {
                self.child = None;
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err("等待托盘 child 退出超时".into());
            }
            thread::sleep(POLL);
        }
    }

    /// 任何等待错误都必须先回收 child，再让 supervisor 删除 LaunchAgent/sandbox。
    pub(crate) fn terminate_and_wait(&mut self) -> Result<(), String> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let mut errors = Vec::new();
        let running = match child.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(error) => {
                errors.push(format!("检查待回收托盘 child 失败：{error}"));
                true
            }
        };
        if running {
            if let Err(error) = child.kill() {
                errors.push(format!("终止待回收托盘 child 失败：{error}"));
            }
        }
        if let Err(error) = child.wait() {
            errors.push(format!("回收托盘 child 失败：{error}"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
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

/// `SystemAutostart::query` 在当前 backend 语义下确认 LaunchAgent entry（plist）状态；
/// 它不等价于 launchd 当前已加载或正在运行该 entry。
pub(crate) fn cleanup_autostart(service: &SystemAutostart, plist: &Path) -> Result<(), String> {
    let operation = service.set(false);
    let entry = service.query();
    if let Err(error) = operation {
        return Err(format!("关闭专用 LaunchAgent 失败：{error}"));
    }
    match entry {
        Ok(false) if !plist.exists() => Ok(()),
        Ok(value) => Err(format!(
            "专用 LaunchAgent entry 清理读回异常：entry={value} plist_exists={}",
            plist.exists(),
        )),
        Err(error) => Err(format!("清理后查询专用 LaunchAgent 失败：{error}")),
    }
}

pub(crate) fn remove_fixture(fixture: &Fixture) -> Result<(), String> {
    fs::remove_dir_all(&fixture.root)
        .map_err(|error| format!("删除托盘探针 sandbox 失败：{error}"))?;
    if fixture.root.exists() {
        return Err("托盘探针 sandbox 删除后仍存在".into());
    }
    Ok(())
}

pub(crate) fn fixture_from_args(args: &[String]) -> Result<Fixture, String> {
    let root = args
        .get(1)
        .map(PathBuf::from)
        .ok_or_else(|| "缺少 child fixture 路径".to_string())?;
    let app_name = args
        .get(2)
        .cloned()
        .ok_or_else(|| "缺少 child LaunchAgent identity".to_string())?;
    let data = root.join("data");
    if !root.is_dir() || !data.is_dir() {
        return Err("child fixture 或 data 目录不存在".into());
    }
    Ok(Fixture {
        root,
        data,
        app_name,
        cleanup_on_drop: false,
    })
}

pub(crate) fn marker(path: &Path, content: impl AsRef<[u8]>) -> Result<(), String> {
    fs::write(path, content).map_err(|error| format!("写 marker {} 失败：{error}", path.display()))
}

pub(crate) fn clear_marker(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("清除 marker {} 失败：{error}", path.display())),
    }
}
