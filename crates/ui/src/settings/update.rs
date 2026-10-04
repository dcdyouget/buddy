//! 设置页「软件更新」（S08-11）：检查、下载、安装，状态与文案对齐 v1 `UpdateSetting`。
//!
//! 与 v1 的差异（用户 2026-10-04 决定）：安装包运行时启动 20 秒后与每 6 小时在后台检查一次，
//! 只把「发现新版本」放到本区域，不弹窗、不自动下载；失败只记日志。
//! 安装完成后发出 [`UpdateReady`]，由 Router 决定立即重启还是等当前回复结束。

use crate::chat_bridge::spawn_engine;
use buddy_update::{Installed, Release, UpdateError};
use gpui::{Bounds, Context, EventEmitter, FocusHandle, Pixels, Task};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_PERIOD: Duration = Duration::from_secs(6 * 60 * 60);
const PROGRESS_TICK: Duration = Duration::from_millis(100);

/// 区域当前展示的状态。
#[derive(Clone, Debug, PartialEq)]
pub enum UpdatePhase {
    /// 尚未检查。
    Idle,
    /// 用户发起的检查进行中。
    Checking,
    /// 当前已是最新版本。
    Latest,
    /// 发现新版本，等待用户确认。
    Available,
    /// 正在下载并校验。
    Downloading,
    /// 正在替换应用。
    Installing,
    /// 已安装，等当前回复完成后重启。
    WaitingIdle,
    /// 正在退出并启动新版本。
    Restarting,
    /// 失败，附带中文提示。
    Failed(String),
}

impl UpdatePhase {
    /// 进行中的状态禁止再次检查或安装。
    pub fn busy(&self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Downloading | Self::Installing | Self::WaitingIdle | Self::Restarting
        )
    }
}

/// 新版本已替换到磁盘，等待重启。
#[derive(Clone, Debug)]
pub struct UpdateReady(pub Installed);

impl EventEmitter<UpdateReady> for UpdateControl {}

/// 软件更新区域。
pub struct UpdateControl {
    pub(super) phase: UpdatePhase,
    pub(super) release: Option<Release>,
    pub(super) active: bool,
    pub(super) check_focus: FocusHandle,
    pub(super) install_focus: FocusHandle,
    pub(super) check_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub(super) install_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub(super) received: Arc<AtomicU64>,
    pub(super) started: Instant,
    client: reqwest::Client,
    task: Option<Task<()>>,
    _schedule: Option<Task<()>>,
}

impl UpdateControl {
    /// 创建区域；仅安装包运行时启动后台检查。
    pub fn new(cx: &mut Context<Self>) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        let schedule = buddy_update::is_installed_app().then(|| {
            cx.spawn(async move |this, cx| {
                let mut delay = FIRST_CHECK;
                loop {
                    cx.background_executor().timer(delay).await;
                    delay = CHECK_PERIOD;
                    let Ok(work) = this.update(cx, |this, cx| this.background_check(cx)) else {
                        return;
                    };
                    if let Some(work) = work {
                        work.await;
                    }
                }
            })
        });
        Self {
            phase: UpdatePhase::Idle,
            release: None,
            active: true,
            check_focus: cx.focus_handle(),
            install_focus: cx.focus_handle(),
            check_bounds: Rc::new(Cell::new(None)),
            install_bounds: Rc::new(Cell::new(None)),
            received: Arc::new(AtomicU64::new(0)),
            started: Instant::now(),
            client,
            task: None,
            _schedule: schedule,
        }
    }

    /// 当前状态（自检读取）。
    pub fn phase(&self) -> &UpdatePhase {
        &self.phase
    }

    /// 设置页退出或子面板打开时释放交互。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active != active {
            self.active = active;
            cx.notify();
        }
    }

    /// 设置页 Tab 导航顺序中的更新按钮。
    pub fn focus_controls(&self) -> Vec<(String, FocusHandle)> {
        if !self.active {
            return Vec::new();
        }
        let mut order = Vec::new();
        if !self.phase.busy() {
            order.push(("update-check".to_owned(), self.check_focus.clone()));
        }
        if self.phase == UpdatePhase::Available {
            order.push(("update-install".to_owned(), self.install_focus.clone()));
        }
        order
    }

    /// 上一帧按钮边界（Tab 导航滚入视口）。
    pub fn control_bounds(&self, id: &str) -> Option<Bounds<Pixels>> {
        match id {
            "update-check" => self.check_bounds.get(),
            "update-install" => self.install_bounds.get(),
            _ => None,
        }
    }

    /// 用户点击「检查更新」。
    pub fn check(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.phase.busy() {
            return;
        }
        self.phase = UpdatePhase::Checking;
        self.release = None;
        let client = self.client.clone();
        let work = spawn_engine(cx, async move { buddy_update::check(&client).await });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.phase = match result {
                    Ok(Some(release)) => {
                        this.release = Some(release);
                        UpdatePhase::Available
                    }
                    Ok(None) => UpdatePhase::Latest,
                    Err(error) => failed("检查更新失败", &error),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// 后台检查不改变可见状态；只有发现新版本且区域空闲时才展示。
    fn background_check(&mut self, cx: &mut Context<Self>) -> Option<Task<()>> {
        if !matches!(self.phase, UpdatePhase::Idle | UpdatePhase::Latest) {
            return None;
        }
        let client = self.client.clone();
        let work = spawn_engine(cx, async move { buddy_update::check(&client).await });
        Some(cx.spawn(async move |this, cx| match work.await {
            Ok(Some(release)) => {
                let _ = this.update(cx, |this, cx| {
                    if matches!(this.phase, UpdatePhase::Idle | UpdatePhase::Latest) {
                        this.release = Some(release);
                        this.phase = UpdatePhase::Available;
                        cx.notify();
                    }
                });
            }
            Ok(None) => {}
            Err(error) => log::info!("后台检查更新失败：{}", error.detail()),
        }))
    }

    /// 用户点击「立即更新」：下载 → 校验 → 替换安装。
    pub fn install(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.phase != UpdatePhase::Available {
            return;
        }
        let Some(release) = self.release.clone() else {
            return;
        };
        self.phase = UpdatePhase::Downloading;
        self.received.store(0, Ordering::Relaxed);
        let client = self.client.clone();
        let received = self.received.clone();
        let download = spawn_engine(cx, async move {
            buddy_update::download(&client, &release, |done, _| {
                received.store(done, Ordering::Relaxed)
            })
            .await
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            // 下载期间定时刷新进度；download 完成后 tick 循环随之结束
            let ticker = cx.spawn({
                let this = this.clone();
                async move |cx| {
                    loop {
                        cx.background_executor().timer(PROGRESS_TICK).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            });
            let downloaded = download.await;
            drop(ticker);
            let downloaded = match downloaded {
                Ok(downloaded) => downloaded,
                Err(error) => return this.update(cx, |this, cx| this.fail("更新失败", &error, cx)).unwrap_or(()),
            };
            if this
                .update(cx, |this, cx| {
                    this.phase = UpdatePhase::Installing;
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
            let installed = cx
                .background_executor()
                .spawn(async move { buddy_update::install(&downloaded) })
                .await;
            let _ = this.update(cx, |this, cx| match installed {
                Ok(installed) => {
                    this.phase = UpdatePhase::Restarting;
                    cx.emit(UpdateReady(installed));
                    cx.notify();
                }
                Err(error) => this.fail("更新失败", &error, cx),
            });
        }));
        cx.notify();
    }

    /// Router：正在流式回复，安装已完成，等回复结束再重启。
    pub fn wait_for_idle(&mut self, cx: &mut Context<Self>) {
        self.phase = UpdatePhase::WaitingIdle;
        cx.notify();
    }

    /// Router：重启失败（新版本已在磁盘上，手动退出再打开即可生效）。
    pub fn restart_failed(&mut self, error: &UpdateError, cx: &mut Context<Self>) {
        self.fail("新版本已安装，但自动重启失败，请手动退出后重新打开", error, cx);
    }

    fn fail(&mut self, prefix: &str, error: &UpdateError, cx: &mut Context<Self>) {
        self.phase = failed(prefix, error);
        cx.notify();
    }
}

fn failed(prefix: &str, error: &UpdateError) -> UpdatePhase {
    log::warn!("{prefix}：{error}（{}）", error.detail());
    UpdatePhase::Failed(format!("{prefix}：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_phases_block_new_actions() {
        for phase in [
            UpdatePhase::Checking,
            UpdatePhase::Downloading,
            UpdatePhase::Installing,
            UpdatePhase::WaitingIdle,
            UpdatePhase::Restarting,
        ] {
            assert!(phase.busy(), "{phase:?}");
        }
        for phase in [
            UpdatePhase::Idle,
            UpdatePhase::Latest,
            UpdatePhase::Available,
            UpdatePhase::Failed("x".into()),
        ] {
            assert!(!phase.busy(), "{phase:?}");
        }
    }

    #[test]
    fn failure_text_is_user_facing_chinese() {
        let phase = failed("检查更新失败", &UpdateError::Network("dns error".into()));
        assert_eq!(
            phase,
            UpdatePhase::Failed("检查更新失败：无法连接更新服务器，请检查网络连接后重试".into())
        );
    }
}
