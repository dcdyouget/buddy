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
}

impl<B: Backend> HotkeyCore<B> {
    pub(crate) fn new(backend: B) -> Self {
        Self {
            backend,
            current: None,
            registered: Vec::new(),
            pressed: HashSet::new(),
        }
    }

    pub(crate) fn update(&mut self, hotkey: HotKey) -> Result<(), HotkeyError> {
        if self.current == Some(hotkey) {
            return Ok(());
        }

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
