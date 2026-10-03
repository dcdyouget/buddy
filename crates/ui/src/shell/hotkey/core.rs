use super::HotkeyError;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use std::collections::HashSet;

pub(crate) trait Backend {
    fn register(&mut self, hotkey: HotKey) -> Result<(), String>;
    fn unregister(&mut self, hotkey: HotKey) -> Result<(), String>;
}

pub(crate) struct HotkeyCore<B> {
    backend: B,
    current: Option<HotKey>,
    registered: Vec<HotKey>,
    pressed: HashSet<u32>,
    /// 在 rearm 已成功注销但重新注册失败时保留待重试的键；不代表 OS 当前有效。
    rearm_pending: Option<HotKey>,
}

impl<B: Backend> HotkeyCore<B> {
    pub(crate) fn new(backend: B) -> Self {
        Self {
            backend,
            current: None,
            registered: Vec::new(),
            pressed: HashSet::new(),
            rearm_pending: None,
        }
    }

    pub(crate) fn update(&mut self, hotkey: HotKey) -> Result<(), HotkeyError> {
        if self.current == Some(hotkey) {
            return Ok(());
        }
        // 新配置取代此前 rearm 失败留下的待重试键；只有真正注册成功后才会
        // 再写入 current，不能用这个待重试值冒充系统已生效。
        self.rearm_pending = None;

        if let Some(old) = self.current {
            // 清理旧 current 之外的遗留注册；失败时尚未触碰新键，旧配置仍有效。
            self.cleanup_except(old)?;
        } else {
            // shutdown 失败可能留下无 current 的注册，重试前先清干净。
            self.cleanup_all()?;
        }

        let Some(old) = self.current else {
            self.backend
                .register(hotkey)
                .map_err(|reason| HotkeyError::Register {
                    hotkey: hotkey.to_string(),
                    reason,
                })?;
            self.registered.push(hotkey);
            self.current = Some(hotkey);
            self.pressed.clear();
            return Ok(());
        };

        self.backend
            .register(hotkey)
            .map_err(|reason| HotkeyError::Register {
                hotkey: hotkey.to_string(),
                reason,
            })?;
        self.registered.push(hotkey);

        if let Err(old_reason) = self.backend.unregister(old) {
            match self.backend.unregister(hotkey) {
                Ok(()) => self.remove_registered(hotkey),
                Err(rollback_reason) => {
                    return Err(HotkeyError::RollbackFailed {
                        old: old.to_string(),
                        old_reason,
                        new: hotkey.to_string(),
                        rollback_reason,
                    });
                }
            }
            return Err(HotkeyError::UnregisterOld {
                hotkey: old.to_string(),
                reason: old_reason,
            });
        }

        self.remove_registered(old);
        self.current = Some(hotkey);
        self.pressed.clear();
        Ok(())
    }

    pub(crate) fn accept(&mut self, event: GlobalHotKeyEvent) -> bool {
        if self.current.is_none_or(|hotkey| hotkey.id() != event.id) {
            return false;
        }

        match event.state {
            HotKeyState::Pressed => self.pressed.insert(event.id),
            HotKeyState::Released => {
                self.pressed.remove(&event.id);
                false
            }
        }
    }

    pub(crate) fn current(&self) -> Option<HotKey> {
        self.current
    }

    pub(crate) fn rearm(&mut self) -> Result<(), HotkeyError> {
        let Some(old) = self.current.or(self.rearm_pending) else {
            self.pressed.clear();
            return Ok(());
        };
        self.cleanup_except(old)?;

        // 注销失败时保留 current、registered 和 pressed，调用方仍可如实认为
        // 旧注册尚未被本进程改变；这里不尝试用内存状态伪造恢复成功。
        if self.current.is_some() {
            if let Err(reason) = self.backend.unregister(old) {
                return Err(HotkeyError::UnregisterOld {
                    hotkey: old.to_string(),
                    reason,
                });
            }

            // 注销已成功，旧键不能继续作为 current；即使后续注册失败也必须保留
            // current = None，避免后续事件路径误认为旧键仍然有效。
            self.remove_registered(old);
            self.current = None;
            self.pressed.clear();
        }

        match self.backend.register(old) {
            Ok(()) => {
                self.registered.push(old);
                self.current = Some(old);
                self.rearm_pending = None;
                self.pressed.clear();
                Ok(())
            }
            Err(reason) => {
                self.current = None;
                self.rearm_pending = Some(old);
                Err(HotkeyError::Register {
                    hotkey: old.to_string(),
                    reason,
                })
            }
        }
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), HotkeyError> {
        let registered = self.registered.clone();
        let mut errors = Vec::new();
        for hotkey in registered {
            if let Err(reason) = self.backend.unregister(hotkey) {
                errors.push(format!("{}：{}", hotkey, reason));
            } else {
                self.remove_registered(hotkey);
            }
        }
        self.current = None;
        self.rearm_pending = None;
        self.pressed.clear();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(HotkeyError::Shutdown(errors))
        }
    }

    fn cleanup_except(&mut self, keep: HotKey) -> Result<(), HotkeyError> {
        let stale = self
            .registered
            .iter()
            .copied()
            .filter(|hotkey| *hotkey != keep)
            .collect::<Vec<_>>();
        for hotkey in stale {
            if let Err(reason) = self.backend.unregister(hotkey) {
                return Err(HotkeyError::Cleanup {
                    hotkey: hotkey.to_string(),
                    reason,
                });
            }
            self.remove_registered(hotkey);
        }
        Ok(())
    }

    fn cleanup_all(&mut self) -> Result<(), HotkeyError> {
        let registered = self.registered.clone();
        for hotkey in registered {
            if let Err(reason) = self.backend.unregister(hotkey) {
                return Err(HotkeyError::Cleanup {
                    hotkey: hotkey.to_string(),
                    reason,
                });
            }
            self.remove_registered(hotkey);
        }
        Ok(())
    }

    fn remove_registered(&mut self, hotkey: HotKey) {
        if let Some(index) = self.registered.iter().position(|item| *item == hotkey) {
            self.registered.remove(index);
        }
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
