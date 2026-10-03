//! S07-11 真实 GPUI 应用退出 hook 探针。
//!
//! child 走产品的 shell::init、主窗口、runtime 和 lifecycle 接线，使用自己
//! 的数据目录与 F19 热键；成功路径只调用 cx.quit()，不使用
//! process::exit(0) 绕过 on_app_quit。

#[cfg(target_os = "macos")]
mod macos_probe {
    use buddy_engine::chat::ChatEngine;
    use buddy_ui::gpui::{App, AsyncApp};
    use buddy_ui::gpui_platform::application;
    use buddy_ui::shell::{
        self,
        config::ShellConfig,
        lifecycle::instance::{Acquire, InstanceError, acquire},
    };
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const TIMEOUT: Duration = Duration::from_secs(15);
    const POLL: Duration = Duration::from_millis(25);
    const HOTKEY: &str = "CmdOrCtrl+Alt+Shift+F19";

    struct Fixture {
        dir: PathBuf,
        data_dir: PathBuf,
        socket: PathBuf,
        ready: PathBuf,
    }

    impl Fixture {
        fn create() -> Result<Self, String> {
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
                data_dir,
                dir,
            })
        }

        fn lock(&self) -> PathBuf {
            self.socket.with_extension("lock")
        }

        fn config_path(&self) -> PathBuf {
            self.data_dir.join("config.json")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    struct OwnedChild {
        child: Option<Child>,
    }

    impl OwnedChild {
        fn spawn(socket: &Path, data_dir: &Path, ready: &Path) -> Result<Self, String> {
            let executable = std::env::current_exe()
                .map_err(|error| format!("读取应用探针路径失败：{error}"))?;
            let child = Command::new(executable)
                .args([
                    "--child".to_owned(),
                    socket.display().to_string(),
                    data_dir.display().to_string(),
                    ready.display().to_string(),
                ])
                .spawn()
                .map_err(|error| format!("启动应用 child 失败：{error}"))?;
            Ok(Self { child: Some(child) })
        }

        fn wait_ready(&mut self, marker: &Path) -> Result<(), String> {
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

        fn wait_success(&mut self) -> Result<(), String> {
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

    pub fn run() -> Result<(), String> {
        let fixture = Fixture::create()?;
        let mut child = OwnedChild::spawn(&fixture.socket, &fixture.data_dir, &fixture.ready)?;
        child.wait_ready(&fixture.ready)?;
        println!("[S07-11] app child READY：lifecycle 已安装，窗口已隐藏");
        child.wait_success()?;
        if fixture.socket.exists() {
            return Err("真实 on_app_quit 后 socket 未清理".into());
        }
        if !fixture.lock().exists() {
            return Err("真实 on_app_quit 后 lock 文件被删除".into());
        }
        match acquire(&fixture.socket) {
            Ok(Acquire::Owner { guard, receiver }) => {
                drop(receiver);
                drop(guard);
            }
            Ok(Acquire::Forwarded) => {
                return Err("真实 child 退出后仍收到 Forwarded".into());
            }
            Err(error) => {
                return Err(format!("真实 child 退出后重新 acquire 失败：{error}"));
            }
        }
        let config = fs::read_to_string(fixture.config_path())
            .map_err(|error| format!("读取 child 最终配置失败：{error}"))?;
        let config = serde_json::from_str::<serde_json::Value>(&config)
            .map_err(|error| format!("解析 child 最终配置失败：{error}"))?;
        if config.get("theme").and_then(serde_json::Value::as_str) != Some("dark") {
            return Err(format!(
                "native quit 后主题未落盘：实际 {:?}，期望 dark",
                config.get("theme")
            ));
        }
        println!(
            "[S07-11] PASS：真实 GPUI cx.quit 触发 on_app_quit，主题 dark 已落盘，socket 清理、lock 保留、再次 acquire 成功"
        );
        Ok(())
    }

    pub fn run_child(args: &[String]) -> Result<(), String> {
        let socket = required_path(args, 1, "socket")?;
        let data_dir = required_path(args, 2, "data")?;
        let ready = required_path(args, 3, "ready")?;
        let acquired = match acquire(&socket).map_err(render_error)? {
            Acquire::Owner { guard, receiver } => (guard, receiver),
            Acquire::Forwarded => return Err("应用 child 意外成为 secondary".into()),
        };
        let (guard, receiver) = acquired;
        let child_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            application()
                .with_assets(buddy_ui::icons::Assets)
                .run(move |cx: &mut App| {
                    shell::init(cx);
                    cx.spawn(async move |cx: &mut AsyncApp| {
                        let engine = ChatEngine::new(data_dir);
                        let handle =
                            match shell::open_main_window(engine, ShellConfig::default(), cx).await
                            {
                                Ok(handle) => handle,
                                Err(error) => fail_child(format!("创建专用主窗口失败：{error}")),
                            };
                        if let Err(error) = shell::runtime::install(handle, cx).await {
                            fail_child(format!("安装专用 F19 runtime 失败：{error}"));
                        }
                        if let Err(error) = shell::runtime::hide(handle, cx).await {
                            fail_child(format!("隐藏专用主窗口失败：{error}"));
                        }
                        if let Err(error) = shell::lifecycle::install(handle, guard, receiver, cx) {
                            fail_child(format!("安装真实生命周期 hook 失败：{error}"));
                        }
                        // 通过真实 SettingsEvent 排队一次配置写入，然后立即请求退出。
                        // 这验证退出协调等待的是实际 Router 配置队列，而非单独的 completion 计数。
                        if let Err(error) = handle.update(cx, |shell, _, cx| {
                            shell.router().update(cx, |router, cx| {
                                router.open_settings(cx);
                                let settings = router.settings_view().clone();
                                settings.update(cx, |_, cx| {
                                    cx.emit(buddy_ui::settings::SettingsEvent::ThemeChanged(
                                        buddy_engine::models::Theme::Dark,
                                    ));
                                });
                            });
                        }) {
                            fail_child(format!("排队主题配置保存失败：{error}"));
                        }
                        fs::write(
                            &ready,
                            format!("READY pid={} hotkey={HOTKEY}\n", std::process::id()),
                        )
                        .unwrap_or_else(|error| {
                            fail_child(format!("写应用 child READY 失败：{error}"))
                        });
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                });
        }));
        if child_result.is_err() {
            return Err("应用 child GPUI 执行 panic".into());
        }
        Ok(())
    }

    fn fail_child(message: String) -> ! {
        eprintln!("[S07-11 app child] FAIL：{message}");
        std::process::exit(1);
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
            eprintln!("[S07-11] FAIL：{error}");
            std::process::exit(1);
        }
    }
}
