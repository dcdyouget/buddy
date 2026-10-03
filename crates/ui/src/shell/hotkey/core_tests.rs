use super::super::{HotkeyError, parse_config};
use super::{Backend, HotkeyCore};
use global_hotkey::hotkey::{CMD_OR_CTRL, Code, HotKey};
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use std::collections::HashSet;

#[derive(Default)]
struct FakeBackend {
    registered: HashSet<HotKey>,
    fail_register: HashSet<HotKey>,
    fail_unregister: HashSet<HotKey>,
    calls: Vec<String>,
}

impl Backend for FakeBackend {
    fn register(&mut self, hotkey: HotKey) -> Result<(), String> {
        self.calls.push(format!("register {hotkey}"));
        if self.fail_register.contains(&hotkey) {
            return Err("模拟冲突".into());
        }
        if !self.registered.insert(hotkey) {
            return Err("已注册".into());
        }
        Ok(())
    }

    fn unregister(&mut self, hotkey: HotKey) -> Result<(), String> {
        self.calls.push(format!("unregister {hotkey}"));
        if self.fail_unregister.contains(&hotkey) {
            return Err("模拟注销失败".into());
        }
        self.registered.remove(&hotkey);
        Ok(())
    }
}

fn key(code: Code) -> HotKey {
    HotKey::new(Some(CMD_OR_CTRL), code)
}

fn event(key: HotKey, state: HotKeyState) -> GlobalHotKeyEvent {
    GlobalHotKeyEvent {
        id: key.id(),
        state,
    }
}

fn with_initial(backend: FakeBackend, first: HotKey) -> HotkeyCore<FakeBackend> {
    let mut core = HotkeyCore::new(backend);
    core.update(first).unwrap();
    core
}

#[test]
fn press_release_are_deduplicated_and_mismatched_ids_ignored() {
    let first = key(Code::KeyJ);
    let other = key(Code::KeyK);
    let mut core = with_initial(FakeBackend::default(), first);

    assert!(core.accept(event(first, HotKeyState::Pressed)));
    assert!(!core.accept(event(first, HotKeyState::Pressed)));
    assert!(!core.accept(event(other, HotKeyState::Released)));
    assert!(!core.accept(event(first, HotKeyState::Released)));
    assert!(!core.accept(event(first, HotKeyState::Released)));
    assert!(core.accept(event(first, HotKeyState::Pressed)));
    assert!(!core.accept(event(first, HotKeyState::Pressed)));
}

#[test]
fn unregistered_and_non_current_pressed_events_are_ignored() {
    let first = key(Code::KeyJ);
    let other = key(Code::KeyK);
    let mut core = HotkeyCore::new(FakeBackend::default());

    assert!(!core.accept(event(first, HotKeyState::Pressed)));
    core.update(first).unwrap();
    assert!(!core.accept(event(other, HotKeyState::Pressed)));
    assert!(!core.accept(event(other, HotKeyState::Released)));
    assert!(core.accept(event(first, HotKeyState::Pressed)));
}

#[test]
fn failed_new_registration_keeps_old_key_and_registration_order() {
    let first = key(Code::KeyJ);
    let second = key(Code::KeyK);
    let mut backend = FakeBackend::default();
    backend.fail_register.insert(second);
    let mut core = with_initial(backend, first);

    assert!(matches!(
        core.update(second),
        Err(HotkeyError::Register { .. })
    ));
    assert_eq!(core.current(), Some(first));
    assert_eq!(core.backend.registered, HashSet::from([first]));
    assert_eq!(core.backend.calls[1], format!("register {second}"));
}

#[test]
fn successful_update_unregisters_old_and_uses_new_id() {
    let first = key(Code::KeyJ);
    let second = key(Code::KeyK);
    let mut core = with_initial(FakeBackend::default(), first);

    assert!(core.update(second).is_ok());
    assert_eq!(core.current(), Some(second));
    assert_eq!(core.backend.registered, HashSet::from([second]));
    assert!(!core.accept(event(first, HotKeyState::Pressed)));
    assert!(core.accept(event(second, HotKeyState::Pressed)));
}

#[test]
fn same_key_update_is_a_no_op() {
    let first = key(Code::KeyJ);
    let mut core = with_initial(FakeBackend::default(), first);

    assert!(core.update(first).is_ok());
    assert_eq!(core.backend.calls, vec![format!("register {first}")]);
}

#[test]
fn rearm_unregisters_and_registers_same_key_and_resets_pressed_state() {
    let first = key(Code::KeyJ);
    let mut core = with_initial(FakeBackend::default(), first);

    assert!(core.accept(event(first, HotKeyState::Pressed)));
    assert!(core.rearm().is_ok());
    assert_eq!(
        core.backend.calls,
        vec![
            format!("register {first}"),
            format!("unregister {first}"),
            format!("register {first}"),
        ]
    );
    assert_eq!(core.current(), Some(first));
    assert_eq!(core.backend.registered, HashSet::from([first]));
    assert!(core.accept(event(first, HotKeyState::Pressed)));
    assert!(!core.accept(event(first, HotKeyState::Pressed)));
}

#[test]
fn rearm_unregister_failure_keeps_old_registration_and_pressed_state() {
    let first = key(Code::KeyJ);
    let mut backend = FakeBackend::default();
    backend.fail_unregister.insert(first);
    let mut core = with_initial(backend, first);

    assert!(core.accept(event(first, HotKeyState::Pressed)));
    assert!(matches!(
        core.rearm(),
        Err(HotkeyError::UnregisterOld { .. })
    ));
    assert_eq!(core.current(), Some(first));
    assert_eq!(core.backend.registered, HashSet::from([first]));
    assert_eq!(
        core.backend.calls,
        vec![format!("register {first}"), format!("unregister {first}")]
    );
    assert!(!core.accept(event(first, HotKeyState::Pressed)));
}

#[test]
fn rearm_register_failure_clears_current_after_successful_unregister() {
    let first = key(Code::KeyJ);
    let mut core = with_initial(FakeBackend::default(), first);
    core.backend.fail_register.insert(first);

    assert!(matches!(core.rearm(), Err(HotkeyError::Register { .. })));
    assert_eq!(core.current(), None);
    assert!(core.backend.registered.is_empty());
    assert_eq!(
        core.backend.calls,
        vec![
            format!("register {first}"),
            format!("unregister {first}"),
            format!("register {first}"),
        ]
    );
    assert!(!core.accept(event(first, HotKeyState::Pressed)));
}

#[test]
fn rearm_can_retry_pending_key_but_reports_failure_until_os_registers_it() {
    let first = key(Code::KeyJ);
    let mut core = with_initial(FakeBackend::default(), first);
    core.backend.fail_register.insert(first);

    assert!(core.rearm().is_err());
    assert_eq!(core.current(), None);
    core.backend.fail_register.remove(&first);
    assert!(core.rearm().is_ok());
    assert_eq!(core.current(), Some(first));
    assert_eq!(core.backend.registered, HashSet::from([first]));
    assert_eq!(
        core.backend.calls,
        vec![
            format!("register {first}"),
            format!("unregister {first}"),
            format!("register {first}"),
            format!("register {first}"),
        ]
    );
}

#[test]
fn failed_old_unregistration_rolls_back_new_key() {
    let first = key(Code::KeyJ);
    let second = key(Code::KeyK);
    let mut backend = FakeBackend::default();
    backend.fail_unregister.insert(first);
    let mut core = with_initial(backend, first);

    assert!(matches!(
        core.update(second),
        Err(HotkeyError::UnregisterOld { .. })
    ));
    assert_eq!(core.current(), Some(first));
    assert_eq!(core.backend.registered, HashSet::from([first]));
    assert_eq!(core.backend.calls[1], format!("register {second}"));
    assert_eq!(core.backend.calls[3], format!("unregister {second}"));
}

#[test]
fn failed_rollback_reports_both_errors_and_tracks_new_registration() {
    let first = key(Code::KeyJ);
    let second = key(Code::KeyK);
    let mut backend = FakeBackend::default();
    backend.fail_unregister.extend([first, second]);
    let mut core = with_initial(backend, first);

    assert!(matches!(
        core.update(second),
        Err(HotkeyError::RollbackFailed { .. })
    ));
    assert_eq!(core.current(), Some(first));
    assert_eq!(core.backend.registered, HashSet::from([first, second]));
}

#[test]
fn cleanup_failure_keeps_old_current_and_does_not_register_new_key() {
    let first = key(Code::KeyJ);
    let second = key(Code::KeyK);
    let third = key(Code::KeyL);
    let mut backend = FakeBackend::default();
    backend.fail_unregister.extend([first, second]);
    let mut core = with_initial(backend, first);
    assert!(matches!(
        core.update(second),
        Err(HotkeyError::RollbackFailed { .. })
    ));

    assert!(matches!(
        core.update(third),
        Err(HotkeyError::Cleanup { .. })
    ));
    assert_eq!(core.current(), Some(first));
    assert_eq!(core.backend.registered, HashSet::from([first, second]));
    assert!(
        !core
            .backend
            .calls
            .iter()
            .any(|call| call == &format!("register {third}"))
    );
}

#[test]
fn shutdown_unregisters_all_keys_and_clears_state() {
    let first = key(Code::KeyJ);
    let mut core = with_initial(FakeBackend::default(), first);

    assert!(core.shutdown().is_ok());
    assert_eq!(core.current(), None);
    assert!(core.backend.registered.is_empty());
    assert!(!core.accept(event(first, HotKeyState::Pressed)));
}

#[test]
fn update_clears_residual_registration_after_failed_shutdown() {
    let first = key(Code::KeyJ);
    let second = key(Code::KeyK);
    let mut backend = FakeBackend::default();
    backend.fail_unregister.insert(first);
    let mut core = with_initial(backend, first);

    assert!(matches!(core.shutdown(), Err(HotkeyError::Shutdown(_))));
    assert_eq!(core.current(), None);
    core.backend.fail_unregister.remove(&first);

    assert!(core.update(second).is_ok());
    assert_eq!(core.current(), Some(second));
    assert_eq!(core.backend.registered, HashSet::from([second]));
}

#[test]
fn invalid_configuration_is_reported_in_chinese_error_type() {
    assert!(matches!(
        parse_config("not-a-real-hotkey"),
        Err(HotkeyError::InvalidConfig(_))
    ));
}
