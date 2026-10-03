//! S07-11 单实例生命周期独立探针。
//!
//! 探针只使用自己创建的 0700 临时目录和自己派生的 child；不会触碰正式
//! Buddy 的 socket、配置或进程。父进程等待 owner READY 后才启动 secondary，
//! 以避免把 owner 尚未完成 bind 的启动窗口误判成第二实例成功。

#[cfg(unix)]
mod unix_probe {
    use buddy_ui::shell::lifecycle::instance::{Acquire, InstanceError, acquire};
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, ExitStatus};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const READY_TIMEOUT: Duration = Duration::from_secs(8);
    const POLL: Duration = Duration::from_millis(20);

    struct Fixture {
        dir: PathBuf,
        socket: PathBuf,
        ready: PathBuf,
        ack: PathBuf,
        release: PathBuf,
        stopped: PathBuf,
        exit: PathBuf,
    }

    impl Fixture {
        fn create() -> Result<Self, String> {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("读取时间失败：{error}"))?
                .as_nanos();
            let dir =
                std::env::temp_dir().join(format!("buddy-s0711-{}-{nonce}", std::process::id()));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .map_err(|error| format!("创建专用 0700 目录失败：{error}"))?;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .map_err(|error| format!("设置专用目录权限失败：{error}"))?;
            Ok(Self {
                socket: dir.join("instance.sock"),
                ready: dir.join("owner.ready"),
                ack: dir.join("owner.ack"),
                release: dir.join("owner.release"),
                stopped: dir.join("owner.stopped"),
                exit: dir.join("owner.exit"),
                dir,
            })
        }

        fn lock(&self) -> PathBuf {
            self.socket.with_extension("lock")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    struct OwnedChild {
        child: Option<Child>,
        label: &'static str,
    }

    impl OwnedChild {
        fn spawn(args: &[String], label: &'static str) -> Result<Self, String> {
            let executable = std::env::current_exe()
                .map_err(|error| format!("读取生命周期探针路径失败：{error}"))?;
            let child = Command::new(executable)
                .args(args)
                .spawn()
                .map_err(|error| format!("启动 {label} child 失败：{error}"))?;
            Ok(Self {
                child: Some(child),
                label,
            })
        }

        fn child_mut(&mut self) -> Result<&mut Child, String> {
            self.child
                .as_mut()
                .ok_or_else(|| format!("{} child 已结束", self.label))
        }

        fn kill_and_wait(&mut self) -> Result<ExitStatus, String> {
            let label = self.label;
            let child = self.child_mut()?;
            if child
                .try_wait()
                .map_err(|error| format!("检查 {label} child 失败：{error}"))?
                .is_none()
            {
                child
                    .kill()
                    .map_err(|error| format!("终止 {label} child 失败：{error}"))?;
            }
            let status = child
                .wait()
                .map_err(|error| format!("等待 {label} child 失败：{error}"))?;
            self.child = None;
            Ok(status)
        }

        fn wait_success(&mut self) -> Result<(), String> {
            let label = self.label;
            let started = std::time::Instant::now();
            loop {
                let status = self
                    .child_mut()?
                    .try_wait()
                    .map_err(|error| format!("等待 {label} child 失败：{error}"))?;
                if let Some(status) = status {
                    self.child = None;
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(format!("{label} child 退出失败：{status}"))
                    };
                }
                if started.elapsed() >= READY_TIMEOUT {
                    return Err(format!("{label} child 退出超时"));
                }
                thread::sleep(POLL);
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

    pub fn run() -> Result<(), String> {
        let fixture = Fixture::create()?;
        println!(
            "[S07-11] fixture={} mode=owner-ready-gated",
            fixture.dir.display()
        );

        let mut owner = OwnedChild::spawn(
            &[
                "--owner-child".into(),
                fixture.socket.display().to_string(),
                fixture.ready.display().to_string(),
                fixture.ack.display().to_string(),
            ],
            "owner",
        )?;
        wait_for_marker(&mut owner, &fixture.ready, "owner READY")?;

        let mut secondary = OwnedChild::spawn(
            &["--secondary".into(), fixture.socket.display().to_string()],
            "secondary",
        )?;
        secondary.wait_success()?;
        wait_for_marker(&mut owner, &fixture.ack, "owner Wake ACK")?;
        println!("[S07-11] owner READY -> secondary Forwarded -> owner Wake ACK PASS");

        let crash_status = owner.kill_and_wait()?;
        if crash_status.success() {
            return Err("owner child 未按崩溃路径退出，拒绝把正常退出当作 crash 证据".into());
        }
        if !fixture.socket.exists() {
            return Err("owner 被强制终止后 socket 已消失，缺少 stale socket 恢复证据".into());
        }

        let recovered_guard = match acquire(&fixture.socket).map_err(render_error)? {
            Acquire::Owner { guard, receiver } => {
                drop(receiver);
                guard
            }
            Acquire::Forwarded => return Err("crash 后仍收到 Forwarded，未成为新 owner".into()),
        };
        println!("[S07-11] crash 后 stale socket 清理并重新成为 owner PASS");
        drop(recovered_guard);
        if fixture.socket.exists() {
            return Err("owner 正常 drop 后 socket 未清理".into());
        }
        if !fixture.lock().exists() {
            return Err("owner 正常 drop 后 lock 文件被删除".into());
        }

        let _ = fs::remove_file(&fixture.ready);
        let _ = fs::remove_file(&fixture.release);
        let _ = fs::remove_file(&fixture.stopped);
        let _ = fs::remove_file(&fixture.exit);
        let mut normal_owner = OwnedChild::spawn(
            &[
                "--normal-owner".into(),
                fixture.socket.display().to_string(),
                fixture.ready.display().to_string(),
                fixture.release.display().to_string(),
                fixture.stopped.display().to_string(),
                fixture.exit.display().to_string(),
            ],
            "normal-owner",
        )?;
        wait_for_marker(&mut normal_owner, &fixture.ready, "normal owner READY")?;
        fs::write(&fixture.release, "RELEASE\n")
            .map_err(|error| format!("写 normal owner release 失败：{error}"))?;
        wait_for_marker(&mut normal_owner, &fixture.stopped, "normal owner stopped")?;
        if fixture.socket.exists() {
            return Err("prepare_process_exit 后 socket 仍存在".into());
        }
        match acquire(&fixture.socket) {
            Err(InstanceError::OwnerUnreachable) => {
                println!("[S07-11] prepare 后 socket 已清理但进程锁仍持有 PASS");
            }
            Ok(Acquire::Owner { guard, receiver }) => {
                drop(receiver);
                drop(guard);
                return Err("prepare_process_exit 后锁已释放，产生并行 owner".into());
            }
            Ok(Acquire::Forwarded) => {
                return Err("prepare_process_exit 后意外 Forwarded".into());
            }
            Err(error) => {
                return Err(format!("prepare_process_exit 锁保持验证失败：{error}"));
            }
        }
        fs::write(&fixture.exit, "EXIT\n")
            .map_err(|error| format!("写 normal owner exit 失败：{error}"))?;
        normal_owner.wait_success()?;
        if fixture.socket.exists() {
            return Err("normal owner 退出后 socket 未清理".into());
        }
        if !fixture.lock().exists() {
            return Err("normal owner 退出后 lock 文件被删除".into());
        }
        println!("[S07-11] normal owner 退出码 0、socket 清理、lock 保留 PASS");

        let final_guard = match acquire(&fixture.socket).map_err(render_error)? {
            Acquire::Owner { guard, receiver } => {
                drop(receiver);
                guard
            }
            Acquire::Forwarded => return Err("socket 清理后重新 acquire 未成为 owner".into()),
        };
        drop(final_guard);
        println!("[S07-11] normal drop socket 清理、lock 保留、再次 acquire PASS");
        Ok(())
    }

    fn wait_for_marker(owner: &mut OwnedChild, marker: &Path, label: &str) -> Result<(), String> {
        let started = std::time::Instant::now();
        while started.elapsed() < READY_TIMEOUT {
            if marker.is_file() {
                return Ok(());
            }
            if let Some(status) = owner
                .child_mut()?
                .try_wait()
                .map_err(|error| format!("等待 {label} 时检查 owner 失败：{error}"))?
            {
                return Err(format!(
                    "{label} 前 owner 已退出：{status}，未越过 READY 门控"
                ));
            }
            thread::sleep(POLL);
        }
        Err(format!("等待 {label} 超时，拒绝启动后续阶段"))
    }

    pub fn run_child(args: &[String]) -> Result<(), String> {
        match args.first().map(String::as_str) {
            Some("--owner-child") => owner_child(args),
            Some("--normal-owner") => normal_owner_child(args),
            Some("--secondary") => secondary_child(args),
            _ => Err("未知生命周期 child 参数".into()),
        }
    }

    fn owner_child(args: &[String]) -> Result<(), String> {
        let socket = required_path(args, 1, "socket")?;
        let ready = required_path(args, 2, "ready")?;
        let ack = required_path(args, 3, "ack")?;
        let (guard, mut receiver) = match acquire(&socket).map_err(render_error)? {
            Acquire::Owner { guard, receiver } => (guard, receiver),
            Acquire::Forwarded => return Err("owner child 意外成为 secondary".into()),
        };
        fs::write(&ready, format!("READY pid={}\n", std::process::id()))
            .map_err(|error| format!("写 owner READY 失败：{error}"))?;
        if receiver.blocking_recv().is_none() {
            return Err("owner listener 在收到 Wake 前结束".into());
        }
        fs::write(&ack, format!("ACK pid={}\n", std::process::id()))
            .map_err(|error| format!("写 owner ACK 失败：{error}"))?;
        let _guard = guard;
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }

    fn secondary_child(args: &[String]) -> Result<(), String> {
        let socket = required_path(args, 1, "socket")?;
        match acquire(&socket).map_err(render_error)? {
            Acquire::Forwarded => {
                println!("[S07-11] secondary Forwarded ACK");
                Ok(())
            }
            Acquire::Owner { guard, receiver } => {
                drop(receiver);
                drop(guard);
                Err("secondary 越过 owner 锁成为新 owner".into())
            }
        }
    }

    fn normal_owner_child(args: &[String]) -> Result<(), String> {
        let socket = required_path(args, 1, "socket")?;
        let ready = required_path(args, 2, "ready")?;
        let release = required_path(args, 3, "release")?;
        let stopped = required_path(args, 4, "stopped")?;
        let exit = required_path(args, 5, "exit")?;
        let (guard, receiver) = match acquire(&socket).map_err(render_error)? {
            Acquire::Owner { guard, receiver } => (guard, receiver),
            Acquire::Forwarded => return Err("normal owner child 意外成为 secondary".into()),
        };
        fs::write(&ready, format!("READY pid={}\n", std::process::id()))
            .map_err(|error| format!("写 normal owner READY 失败：{error}"))?;
        let started = std::time::Instant::now();
        while !release.is_file() {
            if started.elapsed() >= READY_TIMEOUT {
                return Err("等待 normal owner release 超时".into());
            }
            thread::sleep(POLL);
        }
        drop(receiver);
        guard.prepare_process_exit();
        fs::write(&stopped, format!("STOPPED pid={}\n", std::process::id()))
            .map_err(|error| format!("写 normal owner stopped 失败：{error}"))?;
        let started = std::time::Instant::now();
        while !exit.is_file() {
            if started.elapsed() >= READY_TIMEOUT {
                return Err("等待 normal owner exit 超时".into());
            }
            thread::sleep(POLL);
        }
        Ok(())
    }

    fn required_path(args: &[String], index: usize, label: &str) -> Result<PathBuf, String> {
        args.get(index)
            .map(PathBuf::from)
            .ok_or_else(|| format!("缺少 child {label} 路径"))
    }

    fn render_error(error: InstanceError) -> String {
        error.to_string()
    }
}

fn main() {
    #[cfg(not(unix))]
    {
        eprintln!(
            "[S07-11] FAIL：Unix single-instance 探针仅在 macOS/Unix 运行；Windows 归 Phase 09"
        );
        std::process::exit(1);
    }

    #[cfg(unix)]
    {
        if !cfg!(target_os = "macos") {
            eprintln!("[S07-11] FAIL：生命周期探针仅在 macOS 运行；Windows 归 Phase 09");
            std::process::exit(1);
        }
        let args: Vec<String> = std::env::args().skip(1).collect();
        let result = if args.is_empty() {
            unix_probe::run()
        } else {
            unix_probe::run_child(&args)
        };
        if let Err(error) = result {
            eprintln!("[S07-11] FAIL：{error}");
            std::process::exit(1);
        }
        if args.is_empty() {
            println!(
                "[S07-11] PASS：双进程转发、crash stale 恢复、normal drop 清理与 lock 保留均完成"
            );
        }
    }
}
