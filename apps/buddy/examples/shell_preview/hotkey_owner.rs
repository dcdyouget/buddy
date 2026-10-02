//! T47 专用的独立 Carbon 全局热键占用进程。
//!
//! 该 child 使用 `kEventHotKeyExclusive` 独占 B；父测试只有在收到 READY 后才尝试
//! 保存同一热键。Carbon callback 收到事件时写入计数 ACK，用于证明独立 owner 仍持有注册。

use buddy_ui::gpui::App;
use buddy_ui::gpui_platform::application;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

const READY_ENV: &str = "BUDDY_SHELL_HOTKEY_OWNER_READY";
const ACK_ENV: &str = "BUDDY_SHELL_HOTKEY_OWNER_ACK";

pub(crate) struct ChildOwner {
    pub(crate) child: Child,
    pub(crate) ready: PathBuf,
    pub(crate) ack: PathBuf,
}

impl Drop for ChildOwner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.ready);
        let _ = std::fs::remove_file(&self.ack);
    }
}

#[cfg(target_os = "macos")]
mod carbon {
    use super::*;
    use std::ffi::c_void;
    use std::os::raw::c_int;
    use std::ptr;

    type UInt32 = u32;
    type OsStatus = c_int;
    type EventKind = UInt32;
    type EventRef = *mut c_void;
    type EventTargetRef = *mut c_void;
    type EventHandlerRef = *mut c_void;
    type EventHotKeyRef = *mut c_void;
    type EventHandlerCallRef = *mut c_void;

    type EventHandlerProc =
        unsafe extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OsStatus;

    #[repr(C, packed(2))]
    struct EventTypeSpec {
        event_class: UInt32,
        event_kind: EventKind,
    }

    #[repr(C, packed(2))]
    struct EventHotKeyId {
        signature: UInt32,
        id: UInt32,
    }

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn GetApplicationEventTarget() -> EventTargetRef;
        fn InstallEventHandler(
            target: EventTargetRef,
            handler: Option<EventHandlerProc>,
            type_count: usize,
            types: *const EventTypeSpec,
            user_data: *mut c_void,
            out_handler: *mut EventHandlerRef,
        ) -> OsStatus;
        fn RemoveEventHandler(handler: EventHandlerRef) -> OsStatus;
        fn RegisterEventHotKey(
            key_code: UInt32,
            modifiers: UInt32,
            hot_key_id: EventHotKeyId,
            target: EventTargetRef,
            options: UInt32,
            out_hot_key: *mut EventHotKeyRef,
        ) -> OsStatus;
        fn GetEventParameter(
            event: EventRef,
            name: UInt32,
            desired_type: UInt32,
            actual_type: *mut UInt32,
            buffer_size: usize,
            actual_size: *mut usize,
            data: *mut c_void,
        ) -> OsStatus;
        fn GetEventKind(event: EventRef) -> EventKind;
    }

    const NO_ERR: OsStatus = 0;
    const EVENT_CLASS_KEYBOARD: UInt32 = u32::from_be_bytes(*b"keyb");
    const EVENT_HOT_KEY_PRESSED: EventKind = 5;
    const EVENT_HOT_KEY_RELEASED: EventKind = 6;
    const EVENT_PARAM_DIRECT_OBJECT: UInt32 = u32::from_be_bytes(*b"----");
    const TYPE_EVENT_HOT_KEY_ID: UInt32 = u32::from_be_bytes(*b"hkid");
    const HOT_KEY_EXCLUSIVE: UInt32 = 1;

    // Carbon virtual key code for B. The event id is deliberately separate and fixed at 11.
    const B_KEY_CODE: UInt32 = 0x0b;
    const EVENT_ID: UInt32 = 11;
    const SIGNATURE: UInt32 = u32::from_be_bytes(*b"bdky");
    const SHIFT_KEY: UInt32 = 1 << 9;
    const CMD_KEY: UInt32 = 1 << 8;
    const OPTION_KEY: UInt32 = 1 << 11;

    pub(super) struct Registration {
        handler: EventHandlerRef,
        hot_key: EventHotKeyRef,
        state: State,
    }

    struct State {
        ack: Option<PathBuf>,
        count: u64,
    }

    unsafe extern "C" fn callback(
        _call_ref: EventHandlerCallRef,
        event: EventRef,
        user_data: *mut c_void,
    ) -> OsStatus {
        if event.is_null() || user_data.is_null() {
            return NO_ERR;
        }
        let registration = unsafe { &mut *user_data.cast::<Registration>() };
        let mut hot_key_id = EventHotKeyId {
            signature: 0,
            id: 0,
        };
        let status = unsafe {
            GetEventParameter(
                event,
                EVENT_PARAM_DIRECT_OBJECT,
                TYPE_EVENT_HOT_KEY_ID,
                ptr::null_mut(),
                std::mem::size_of::<EventHotKeyId>(),
                ptr::null_mut(),
                (&mut hot_key_id as *mut EventHotKeyId).cast(),
            )
        };
        if status != NO_ERR {
            return NO_ERR;
        }
        let id = unsafe { ptr::addr_of!(hot_key_id.id).read_unaligned() };
        let signature = unsafe { ptr::addr_of!(hot_key_id.signature).read_unaligned() };
        let kind = unsafe { GetEventKind(event) };
        if id == EVENT_ID
            && signature == SIGNATURE
            && (kind == EVENT_HOT_KEY_PRESSED || kind == EVENT_HOT_KEY_RELEASED)
        {
            registration.state.count += 1;
            if let Some(path) = registration.state.ack.as_ref() {
                let _ = std::fs::write(
                    path,
                    format!("ACK {} id={} kind={}", registration.state.count, id, kind),
                );
            }
        }
        NO_ERR
    }

    pub(super) fn register(ack: Option<PathBuf>) -> Result<Box<Registration>, OsStatus> {
        let mut registration = Box::new(Registration {
            handler: ptr::null_mut(),
            hot_key: ptr::null_mut(),
            state: State { ack, count: 0 },
        });
        let registration_ptr: *mut Registration = &mut *registration;
        let types = [
            EventTypeSpec {
                event_class: EVENT_CLASS_KEYBOARD,
                event_kind: EVENT_HOT_KEY_PRESSED,
            },
            EventTypeSpec {
                event_class: EVENT_CLASS_KEYBOARD,
                event_kind: EVENT_HOT_KEY_RELEASED,
            },
        ];
        let target = unsafe { GetApplicationEventTarget() };
        let status = unsafe {
            InstallEventHandler(
                target,
                Some(callback),
                types.len(),
                types.as_ptr(),
                registration_ptr.cast(),
                &mut (*registration_ptr).handler,
            )
        };
        if status != NO_ERR {
            return Err(status);
        }
        let hot_key_id = EventHotKeyId {
            signature: SIGNATURE,
            id: EVENT_ID,
        };
        let status = unsafe {
            RegisterEventHotKey(
                B_KEY_CODE,
                CMD_KEY | OPTION_KEY | SHIFT_KEY,
                hot_key_id,
                target,
                HOT_KEY_EXCLUSIVE,
                &mut (*registration_ptr).hot_key,
            )
        };
        if status != NO_ERR {
            unsafe { RemoveEventHandler((*registration_ptr).handler) };
            return Err(status);
        }
        Ok(registration)
    }
}

/// 由 shell_preview 主入口在 `--hotkey-owner` 下调用。
pub(crate) fn run_child() -> ! {
    #[cfg(target_os = "macos")]
    application().run(|cx: &mut App| {
        buddy_ui::shell::init(cx);
        let ready = std::env::var_os(READY_ENV).map(PathBuf::from);
        let ack = std::env::var_os(ACK_ENV).map(PathBuf::from);
        let Ok(registration) = carbon::register(ack) else {
            eprintln!("hotkey owner: Carbon 独占注册 B 失败");
            return;
        };
        // Keep the Carbon handler and hotkey ref alive with the AppKit event loop.
        let _registration = Box::leak(registration);
        if let Some(path) = ready.as_ref() {
            let _ = std::fs::write(path, "READY B EXCLUSIVE");
        }
    });
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("hotkey owner requires macOS");
    }
    std::process::exit(0)
}

/// 启动同一个示例的专用 Carbon 独占 child。父测试结束时通过 Drop 清理进程和标记文件。
pub(crate) fn spawn() -> std::io::Result<ChildOwner> {
    let root = std::env::temp_dir().join(format!(
        "buddy-shell-preview-hotkey-owner-{}",
        std::process::id()
    ));
    let ready = root.with_extension("ready");
    let ack = root.with_extension("ack");
    let _ = std::fs::remove_file(&ready);
    let _ = std::fs::remove_file(&ack);
    let child = Command::new(std::env::current_exe()?)
        .arg("--hotkey-owner")
        .env(READY_ENV, &ready)
        .env(ACK_ENV, &ack)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ChildOwner { child, ready, ack })
}
