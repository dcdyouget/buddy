//! macOS LaunchAgent 开机自启服务（S07-10）。
//!
//! 该模块只负责真实系统登录项的查询和切换。托盘菜单、配置写盘和用户提示由
//! 上层调用者负责；因此可以在上层注入 [`AutostartBackend`]，而不会让单测修改
//! 当前用户的登录项。

use std::path::{Path, PathBuf};

/// 可注入的开机自启后端。
///
/// 上层应先调用 [`AutostartBackend::query`] 保存旧状态，再调用 `set`；配置写盘
/// 失败时由上层调用 `set(old_state)` 恢复系统状态。实现不得把配置文件值当作
/// 系统查询结果。
pub trait AutostartBackend {
    /// 读取系统当前是否已注册 Buddy 的登录项。
    fn query(&self) -> Result<bool, String>;

    /// 设置系统登录项，并读回确认目标状态。
    fn set(&self, enabled: bool) -> Result<(), String>;
}

/// 使用 macOS LaunchAgent 的真实开机自启后端。
pub struct SystemAutostart {
    #[cfg(target_os = "macos")]
    launcher: auto_launch::AutoLaunch,
}

impl SystemAutostart {
    /// 构造正式 Buddy 后端，使用当前进程的绝对可执行文件路径。
    pub fn new() -> Result<Self, String> {
        let path = current_executable_path()?;
        Self::with_identity("Buddy", &path)
    }

    /// 使用指定名称和绝对路径构造后端。
    ///
    /// 正式产品使用 [`Self::new`]；独立探针使用唯一名称，避免触碰真实 Buddy
    /// 的 LaunchAgent。
    pub fn with_identity(app_name: &str, app_path: &Path) -> Result<Self, String> {
        if app_name.is_empty() || app_name.contains('/') || app_name.contains('\\') {
            return Err("开机自启名称不能为空且不能包含路径分隔符".into());
        }
        if !app_path.is_absolute() {
            return Err("开机自启可执行文件路径必须是绝对路径".into());
        }

        #[cfg(target_os = "macos")]
        {
            let path = app_path.to_string_lossy();
            let launcher = auto_launch::AutoLaunchBuilder::new()
                .set_app_name(app_name)
                .set_app_path(&path)
                .set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent)
                .build()
                .map_err(|error| format!("构造 macOS 开机自启失败：{error}"))?;
            Ok(Self { launcher })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app_name, app_path);
            Err("S07-10 当前仅实现 macOS LaunchAgent；Windows 归 Phase 09".into())
        }
    }

    /// 查询 macOS 当前登录项状态。
    pub fn query(&self) -> Result<bool, String> {
        #[cfg(target_os = "macos")]
        {
            self.launcher
                .is_enabled()
                .map_err(|error| format!("查询 macOS 开机自启失败：{error}"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("S07-10 当前仅实现 macOS LaunchAgent；Windows 归 Phase 09".into())
        }
    }

    /// 设置 macOS 登录项，并读回确认；失败或读回不一致时恢复原状态。
    pub fn set(&self, enabled: bool) -> Result<(), String> {
        set_with_rollback(self, enabled)
    }
}

impl AutostartBackend for SystemAutostart {
    fn query(&self) -> Result<bool, String> {
        Self::query(self)
    }

    fn set(&self, enabled: bool) -> Result<(), String> {
        Self::set(self, enabled)
    }
}

trait RawAutostartBackend {
    fn query_raw(&self) -> Result<bool, String>;
    fn apply_raw(&self, enabled: bool) -> Result<(), String>;
}

impl RawAutostartBackend for SystemAutostart {
    fn query_raw(&self) -> Result<bool, String> {
        self.query()
    }

    fn apply_raw(&self, enabled: bool) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let result = if enabled {
                self.launcher.enable()
            } else {
                self.launcher.disable()
            };
            result.map_err(|error| {
                format!(
                    "{} macOS 开机自启失败：{error}",
                    if enabled { "开启" } else { "关闭" }
                )
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = enabled;
            Err("S07-10 当前仅实现 macOS LaunchAgent；Windows 归 Phase 09".into())
        }
    }
}

fn set_with_rollback<B: RawAutostartBackend>(backend: &B, desired: bool) -> Result<(), String> {
    let before = backend
        .query_raw()
        .map_err(|error| format!("读取开机自启原状态失败：{error}"))?;
    if before == desired {
        return Ok(());
    }

    if let Err(error) = backend.apply_raw(desired) {
        return Err(with_rollback(
            backend,
            before,
            format!(
                "{}开机自启失败：{error}",
                if desired { "开启" } else { "关闭" }
            ),
        ));
    }

    match backend.query_raw() {
        Ok(actual) if actual == desired => Ok(()),
        Ok(actual) => Err(with_rollback(
            backend,
            before,
            format!("开机自启读回不一致：期望 {desired}，实际 {actual}"),
        )),
        Err(error) => Err(with_rollback(
            backend,
            before,
            format!("开机自启设置后查询失败：{error}"),
        )),
    }
}

fn with_rollback<B: RawAutostartBackend>(backend: &B, before: bool, reason: String) -> String {
    if let Err(error) = backend.apply_raw(before) {
        return format!("{reason}；恢复原开机自启状态失败：{error}");
    }
    match backend.query_raw() {
        Ok(actual) if actual == before => format!("{reason}；已恢复原开机自启状态"),
        Ok(actual) => format!("{reason}；恢复后读回不一致：期望 {before}，实际 {actual}"),
        Err(error) => format!("{reason}；恢复后查询失败：{error}"),
    }
}

fn current_executable_path() -> Result<PathBuf, String> {
    let path =
        std::env::current_exe().map_err(|error| format!("读取当前可执行文件失败：{error}"))?;
    if !path.is_absolute() {
        return Err("当前可执行文件路径不是绝对路径".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{RawAutostartBackend, set_with_rollback};
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;

    struct FakeBackend {
        state: Cell<bool>,
        failures: RefCell<Vec<bool>>,
        reports: RefCell<VecDeque<Result<bool, String>>>,
        applied: RefCell<Vec<bool>>,
    }

    impl FakeBackend {
        fn new(state: bool) -> Self {
            Self {
                state: Cell::new(state),
                failures: RefCell::new(Vec::new()),
                reports: RefCell::new(VecDeque::new()),
                applied: RefCell::new(Vec::new()),
            }
        }
    }

    impl RawAutostartBackend for FakeBackend {
        fn query_raw(&self) -> Result<bool, String> {
            self.reports
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Ok(self.state.get()))
        }

        fn apply_raw(&self, enabled: bool) -> Result<(), String> {
            self.applied.borrow_mut().push(enabled);
            if self.failures.borrow().contains(&enabled) {
                return Err("测试故意失败".into());
            }
            self.state.set(enabled);
            Ok(())
        }
    }

    #[test]
    fn already_matching_state_does_not_touch_os() {
        let backend = FakeBackend::new(false);
        assert!(set_with_rollback(&backend, false).is_ok());
        assert!(backend.applied.borrow().is_empty());
    }

    #[test]
    fn apply_failure_attempts_to_restore_original_state() {
        let backend = FakeBackend::new(false);
        backend.failures.borrow_mut().push(true);
        let error = set_with_rollback(&backend, true).unwrap_err();
        assert!(error.contains("恢复原开机自启状态"));
        assert!(!backend.state.get());
        assert_eq!(&*backend.applied.borrow(), &[true, false]);
    }

    #[test]
    fn rollback_failure_is_reported_instead_of_claiming_restoration() {
        let backend = FakeBackend::new(false);
        *backend.failures.borrow_mut() = vec![true, false];
        let error = set_with_rollback(&backend, true).unwrap_err();
        assert!(error.contains("恢复原开机自启状态失败"));
        assert_eq!(&*backend.applied.borrow(), &[true, false]);
    }

    #[test]
    fn readback_mismatch_fails_and_restores_original_state() {
        let backend = FakeBackend::new(false);
        *backend.reports.borrow_mut() = VecDeque::from([Ok(false), Ok(false), Ok(false)]);
        let error = set_with_rollback(&backend, true).unwrap_err();
        assert!(error.contains("读回不一致"));
        assert!(error.contains("已恢复原开机自启状态"));
        assert!(!backend.state.get());
        assert_eq!(&*backend.applied.borrow(), &[true, false]);
    }

    #[test]
    fn post_apply_query_failure_is_intercepted_and_restored() {
        let backend = FakeBackend::new(false);
        *backend.reports.borrow_mut() =
            VecDeque::from([Ok(false), Err("测试设置后查询失败".into()), Ok(false)]);
        let error = set_with_rollback(&backend, true).unwrap_err();
        assert!(error.contains("设置后查询失败"));
        assert!(error.contains("已恢复原开机自启状态"));
        assert!(!backend.state.get());
        assert_eq!(&*backend.applied.borrow(), &[true, false]);
    }

    #[test]
    fn initial_query_failure_does_not_mutate_os() {
        let backend = FakeBackend::new(false);
        *backend.reports.borrow_mut() = VecDeque::from([Err("测试查询失败".into())]);
        let error = set_with_rollback(&backend, true).unwrap_err();
        assert!(error.contains("读取开机自启原状态失败"));
        assert!(backend.applied.borrow().is_empty());
    }
}
