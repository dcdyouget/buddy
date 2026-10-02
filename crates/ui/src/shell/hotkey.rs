//! 全局热键注册与事件桥接（S07-03）。
//!
//! `GlobalHotKeyManager` 必须由 GPUI 主线程创建并保持存活。操作系统事件由
//! global-hotkey 的一次性 handler 原样送入 tokio channel；GPUI 主线程再调用
//! [`GlobalHotkey::accept`] 做当前热键匹配和 Pressed / Released 去重。

#[path = "hotkey/core.rs"]
mod core;

use core::{Backend, HotkeyCore};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc::{self, UnboundedReceiver};

/// global-hotkey 事件桥的接收端；事件内容保持库的原始值。
pub type HotkeyReceiver = UnboundedReceiver<GlobalHotKeyEvent>;

/// 全局热键操作失败时的中文诊断。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyError {
    /// 进程已经安装过唯一的 global-hotkey 事件 handler。
    AlreadyInitialized,
    /// 配置字符串无法解析为 global-hotkey 的 HotKey。
    InvalidConfig(String),
    /// 创建操作系统热键管理器失败。
    Manager(String),
    /// 注册新热键失败，旧热键尚未修改。
    Register {
        /// 申请注册的热键。
        hotkey: String,
        /// 操作系统返回的失败原因。
        reason: String,
    },
    /// 旧热键注销失败，尝试回滚新热键。
    UnregisterOld {
        /// 未能注销的旧热键。
        hotkey: String,
        /// 操作系统返回的失败原因。
        reason: String,
    },
    /// 旧热键注销失败且新热键回滚也失败。
    RollbackFailed {
        /// 未能注销的旧热键。
        old: String,
        /// 旧热键注销失败原因。
        old_reason: String,
        /// 未能回滚的新热键。
        new: String,
        /// 回滚新热键失败原因。
        rollback_reason: String,
    },
    /// 清理此前遗留的注册失败。
    Cleanup {
        /// 未能清理的遗留热键。
        hotkey: String,
        /// 操作系统返回的失败原因。
        reason: String,
    },
    /// 关闭时有一个或多个热键注销失败。
    Shutdown(Vec<String>),
    /// 事件接收端已被取走或其锁已中毒。
    EventBridgeUnavailable,
}

impl std::fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyInitialized => f.write_str("全局热键事件桥已经初始化"),
            Self::InvalidConfig(value) => write!(f, "无效的全局热键配置：{value}"),
            Self::Manager(reason) => write!(f, "创建全局热键管理器失败：{reason}"),
            Self::Register { hotkey, reason } => {
                write!(f, "注册全局热键 {hotkey} 失败：{reason}")
            }
            Self::UnregisterOld { hotkey, reason } => {
                write!(f, "注销旧全局热键 {hotkey} 失败，已保留旧注册：{reason}")
            }
            Self::RollbackFailed {
                old,
                old_reason,
                new,
                rollback_reason,
            } => write!(
                f,
                "注销旧全局热键 {old} 失败（{old_reason}），回滚新热键 {new} 也失败（{rollback_reason}）"
            ),
            Self::Cleanup { hotkey, reason } => {
                write!(f, "清理遗留全局热键 {hotkey} 失败：{reason}")
            }
            Self::Shutdown(errors) => write!(f, "关闭全局热键时有注销失败：{}", errors.join("；")),
            Self::EventBridgeUnavailable => f.write_str("全局热键事件桥接收端不可用"),
        }
    }
}

impl std::error::Error for HotkeyError {}

struct ManagerBackend {
    manager: GlobalHotKeyManager,
}

impl Backend for ManagerBackend {
    fn register(&mut self, hotkey: global_hotkey::hotkey::HotKey) -> Result<(), String> {
        self.manager
            .register(hotkey)
            .map_err(|error| error.to_string())
    }

    fn unregister(&mut self, hotkey: global_hotkey::hotkey::HotKey) -> Result<(), String> {
        self.manager
            .unregister(hotkey)
            .map_err(|error| error.to_string())
    }
}

/// 进程唯一的事件桥。global-hotkey 自身的 handler 是 OnceCell，因此这里也只安装一次。
static EVENT_BRIDGE: OnceLock<Mutex<Option<HotkeyReceiver>>> = OnceLock::new();

fn take_event_receiver() -> Result<HotkeyReceiver, HotkeyError> {
    if EVENT_BRIDGE.get().is_some() {
        return Err(HotkeyError::AlreadyInitialized);
    }
    let (sender, receiver) = mpsc::unbounded_channel();
    let handler_sender = sender.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        let _ = handler_sender.send(event);
    }));

    EVENT_BRIDGE
        .set(Mutex::new(Some(receiver)))
        .map_err(|_| HotkeyError::AlreadyInitialized)?;
    EVENT_BRIDGE
        .get()
        .and_then(|bridge| bridge.lock().ok()?.take())
        .ok_or(HotkeyError::EventBridgeUnavailable)
}

/// 主线程持有的全局热键管理器。
pub struct GlobalHotkey {
    core: HotkeyCore<ManagerBackend>,
}

impl GlobalHotkey {
    /// 在主线程创建管理器、注册初始配置，并返回无轮询事件接收端。
    pub fn new(config: &str) -> Result<(Self, HotkeyReceiver), HotkeyError> {
        let (mut hotkey, receiver) = Self::unregistered()?;
        hotkey.register(config)?;
        Ok((hotkey, receiver))
    }

    /// 在主线程创建未注册热键的持久管理器和事件接收端。
    ///
    /// 启动配置冲突时使用此入口，可保留 manager 并让设置页修改后重试。
    pub fn unregistered() -> Result<(Self, HotkeyReceiver), HotkeyError> {
        let manager =
            GlobalHotKeyManager::new().map_err(|error| HotkeyError::Manager(error.to_string()))?;
        let core = HotkeyCore::new(ManagerBackend { manager });
        let receiver = take_event_receiver()?;
        Ok((Self { core }, receiver))
    }

    /// 注册或更新配置；保留此名称供窗口初始化代码使用。
    pub fn register(&mut self, config: &str) -> Result<(), HotkeyError> {
        self.update(config)
    }

    /// 先注册新热键，再注销旧热键；失败时尽量恢复旧注册。
    pub fn update(&mut self, config: &str) -> Result<(), HotkeyError> {
        self.core.update(parse_config(config)?)
    }

    /// 接受一个原始系统事件；返回 true 表示应触发一次切换动作。
    pub fn accept(&mut self, event: GlobalHotKeyEvent) -> bool {
        self.core.accept(event)
    }

    /// 当前生效的热键。
    pub fn current(&self) -> Option<global_hotkey::hotkey::HotKey> {
        self.core.current()
    }

    /// 注销当前及恢复列表中的所有热键。
    pub fn shutdown(mut self) -> Result<(), HotkeyError> {
        self.core.shutdown()
    }
}

fn parse_config(config: &str) -> Result<global_hotkey::hotkey::HotKey, HotkeyError> {
    config
        .trim()
        .parse::<global_hotkey::hotkey::HotKey>()
        .map_err(|error| HotkeyError::InvalidConfig(error.to_string()))
}
