//! S07-10 真实 LaunchAgent 配置写盘失败回滚。

use super::*;
use buddy_ui::shell::autostart::SystemAutostart;

struct ObservedAutostart {
    service: Rc<SystemAutostart>,
    enabled_readback: Cell<bool>,
}
impl AutostartBackend for ObservedAutostart {
    fn query(&self) -> Result<bool, String> {
        self.service.query()
    }
    fn set(&self, enabled: bool) -> Result<(), String> {
        self.service.set(enabled)?;
        if enabled {
            self.enabled_readback.set(self.service.query()?);
        }
        Ok(())
    }
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

/// 使用唯一专用 LaunchAgent 验证 Router 写盘失败后真实 OS 状态恢复。
pub(crate) async fn run(cx: &mut AsyncApp, root: &Path) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("真实 LaunchAgent 回滚自测仅在 macOS 运行".into());
    }

    let app_name = format!("BuddyS0710TransactionProbe-{}", std::process::id());
    let plist = launch_agent_path(&app_name)?;
    if plist.exists() {
        return Err(format!(
            "专用 LaunchAgent 已存在，拒绝覆盖：{}",
            plist.display()
        ));
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("读取 preferences 自测路径失败：{error}"))?;
    let service = Rc::new(SystemAutostart::with_identity(&app_name, &executable)?);
    if service.query()? {
        return Err("专用 LaunchAgent 初始已开启，拒绝改变未知状态".into());
    }

    let operation = async {
        let dir = super::sandbox(root, "native-save-failure")?;
        let initial = super::config();
        storage::save_config(&dir, &initial)?;
        let before_bytes = fs::read(dir.join("config.json"))
            .map_err(|error| format!("读取真实回滚样本失败：{error}"))?;
        let handle = cx.update(|app| super::open_router(app, dir.clone(), initial));
        let observed = Rc::new(ObservedAutostart {
            service: service.clone(),
            enabled_readback: Cell::new(false),
        });
        let backend: Rc<dyn AutostartBackend> = observed.clone();
        let mut guard = super::ConfigPathGuard::block(dir.join("config.json"))?;
        let result = super::toggle(handle, backend, cx).await;
        let restore = guard.restore();
        drop(guard);
        restore?;

        let after_bytes = fs::read(dir.join("config.json"))
            .map_err(|error| format!("读取恢复后配置失败：{error}"))?;
        let disk = storage::get_config(&dir)?;
        let visible = super::router_config(handle, cx)?;
        let os_after_failure = service.query()?;
        if !result.as_ref().is_err_and(|error| error.contains("保存开机自启失败"))
            || !observed.enabled_readback.get()
            || os_after_failure
            || disk.auto_start
            || visible.auto_start
            || before_bytes != after_bytes
        {
            return Err(format!(
                "真实回滚断言失败：result={result:?} os={os_after_failure} disk={} visible={} bytes_equal={}",
                disk.auto_start,
                visible.auto_start,
                before_bytes == after_bytes
            ));
        }
        println!(
            "PASS S07-10 preferences native-save-rollback enabled_readback=true result_err={} os={} disk={} visible={} bytes_equal=true",
            result.is_err(),
            os_after_failure,
            disk.auto_start,
            visible.auto_start
        );
        Ok(())
    }
    .await;

    // 专用 identity 已由本测试拥有；无论 Router 或 engine 结果如何都清理。
    let cleanup = service.set(false);
    let final_state = service.query();
    let plist_exists = plist.exists();
    match (operation, cleanup, final_state, plist_exists) {
        (Ok(()), Ok(()), Ok(false), false) => Ok(()),
        (result, cleanup, state, exists) => Err(format!(
            "真实专用 LaunchAgent 清理失败：operation={result:?} cleanup={cleanup:?} final_state={state:?} plist_exists={exists}"
        )),
    }
}
