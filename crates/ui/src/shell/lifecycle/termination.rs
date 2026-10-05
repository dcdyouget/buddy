//! macOS application termination coordination.
//!
//! GPUI's macOS delegate handles applicationWillTerminate: but does not expose
//! applicationShouldTerminate:. This module adds that selector to the existing
//! delegate class at runtime without replacing any existing implementation.
//! AppKit receives NSTerminateCancel so its modal termination loop cannot stall
//! GPUI dispatch. After async cleanup, an approved retry returns NSTerminateNow.

#![cfg(target_os = "macos")]

use objc::runtime::{BOOL, Class, Object, Sel, YES, object_getClass};
use objc::{sel, sel_impl};
use std::ffi::c_char;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

const NS_TERMINATE_CANCEL: isize = 0;
static TERMINATION_APPROVED: AtomicBool = AtomicBool::new(false);
const OBJC_METHOD_TYPES: &[u8] = b"q@:@\0";

static TERMINATION_SENDER: OnceLock<Mutex<Option<UnboundedSender<TerminationRequest>>>> =
    OnceLock::new();
static TERMINATION_PENDING: AtomicBool = AtomicBool::new(false);

/// A request from AppKit to finish cleanup and terminate the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminationRequest;

/// Errors while installing or replying to the AppKit termination hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationError {
    /// The call must happen on the AppKit main thread.
    NotMainThread,
    /// NSApplication did not provide a shared application object.
    MissingApplication,
    /// NSApplication did not provide a delegate object.
    MissingDelegate,
    /// The termination hook was already installed by this process.
    AlreadyInstalled,
    /// The delegate already implements this selector; refusing to overwrite it.
    SelectorAlreadyExists,
    /// Objective-C rejected adding the selector to the delegate class.
    SelectorInstallFailed,
}

impl std::fmt::Display for TerminationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotMainThread => f.write_str("macOS 退出 hook 必须在主线程安装或回复"),
            Self::MissingApplication => f.write_str("无法取得 NSApplication"),
            Self::MissingDelegate => f.write_str("NSApplication 没有 delegate"),
            Self::AlreadyInstalled => f.write_str("macOS 退出 hook 已安装"),
            Self::SelectorAlreadyExists => {
                f.write_str("delegate 已存在 applicationShouldTerminate:，拒绝覆盖")
            }
            Self::SelectorInstallFailed => {
                f.write_str("向 NSApplication delegate 安装 applicationShouldTerminate: 失败")
            }
        }
    }
}

impl std::error::Error for TerminationError {}

/// Install the delegate hook and return the main-thread termination stream.
///
/// The selector is checked on the concrete delegate class, including inherited
/// methods, before class_addMethod; an existing implementation is never
/// replaced. Calling this more than once returns AlreadyInstalled.
pub fn install() -> Result<UnboundedReceiver<TerminationRequest>, TerminationError> {
    ensure_main_thread()?;
    if TERMINATION_SENDER.get().is_some() {
        return Err(TerminationError::AlreadyInstalled);
    }

    let application = shared_application()?;
    let delegate: *mut Object = unsafe { objc::msg_send![application, delegate] };
    if delegate.is_null() {
        return Err(TerminationError::MissingDelegate);
    }
    let delegate_class = unsafe { object_getClass(delegate.cast_const()) };
    if delegate_class.is_null() {
        return Err(TerminationError::MissingDelegate);
    }
    let selector = Sel::register("applicationShouldTerminate:");
    // SAFETY: delegate_class is the live Objective-C class of NSApplication's
    // delegate and selector is a registered immutable selector.
    let existing = unsafe { objc::runtime::class_getInstanceMethod(delegate_class, selector) };
    if !existing.is_null() {
        return Err(TerminationError::SelectorAlreadyExists);
    }

    let implementation: objc::runtime::Imp = unsafe {
        std::mem::transmute::<
            unsafe extern "C" fn(&mut Object, Sel, *mut Object) -> isize,
            objc::runtime::Imp,
        >(application_should_terminate)
    };
    // SAFETY: the class is mutable, the method encoding matches the callback
    // (NSInteger, self, _cmd, and one object argument), and no existing
    // selector was found above.
    let added = unsafe {
        objc::runtime::class_addMethod(
            delegate_class as *mut Class,
            selector,
            implementation,
            OBJC_METHOD_TYPES.as_ptr() as *const c_char,
        )
    };
    if added != YES {
        return Err(TerminationError::SelectorInstallFailed);
    }

    let (sender, receiver) = mpsc::unbounded_channel();
    if TERMINATION_SENDER.set(Mutex::new(Some(sender))).is_err() {
        // A second installation can only race through an externally-created
        // main-thread call. Never replace the existing sender.
        return Err(TerminationError::AlreadyInstalled);
    }
    Ok(receiver)
}

/// Retry termination after the main thread has drained the configuration queue.
pub fn reply_to_application_should_terminate() -> Result<(), TerminationError> {
    ensure_main_thread()?;
    let application = shared_application()?;
    TERMINATION_APPROVED.store(true, Ordering::Release);
    // No GPUI App borrow may be held: termination synchronously runs its hooks.
    unsafe {
        let _: () = objc::msg_send![application, terminate: std::ptr::null_mut::<Object>()];
    }
    Ok(())
}

unsafe extern "C" fn application_should_terminate(
    _delegate: &mut Object,
    _selector: Sel,
    _application: *mut Object,
) -> isize {
    if TERMINATION_APPROVED.swap(false, Ordering::AcqRel) {
        return 1; // NSTerminateNow
    }
    if !TERMINATION_PENDING.swap(true, Ordering::AcqRel) {
        log_termination_source();
        if let Some(channel) = TERMINATION_SENDER.get() {
            let guard = channel
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let send_failed = guard
                .as_ref()
                .map(|sender| sender.send(TerminationRequest).is_err())
                .unwrap_or(true);
            if send_failed {
                TERMINATION_PENDING.store(false, Ordering::Release);
                return 0; // NSTerminateCancel: no receiver can reply.
            }
        } else {
            TERMINATION_PENDING.store(false, Ordering::Release);
            return 0;
        }
    }
    NS_TERMINATE_CANCEL
}

/// 记录是谁请求退出：当前输入事件、Apple Event（含发送方进程）与调用栈。
/// 退出码为 0 的「不明退出」只能靠这里区分 ⌘Q、菜单、托盘、其他进程发来的退出事件等来源。
fn log_termination_source() {
    unsafe fn describe(object: *mut Object) -> String {
        if object.is_null() {
            return "无".into();
        }
        unsafe {
            let text: *mut Object = objc::msg_send![object, description];
            if text.is_null() {
                return "无".into();
            }
            let utf8: *const c_char = objc::msg_send![text, UTF8String];
            if utf8.is_null() {
                return "无".into();
            }
            std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned()
        }
    }
    let (event, apple_event) = unsafe {
        let application: *mut Object =
            objc::msg_send![objc::class!(NSApplication), sharedApplication];
        let event: *mut Object = if application.is_null() {
            std::ptr::null_mut()
        } else {
            objc::msg_send![application, currentEvent]
        };
        let manager: *mut Object =
            objc::msg_send![objc::class!(NSAppleEventManager), sharedAppleEventManager];
        let apple_event: *mut Object = if manager.is_null() {
            std::ptr::null_mut()
        } else {
            objc::msg_send![manager, currentAppleEvent]
        };
        (describe(event), describe(apple_event))
    };
    log::warn!(
        "[退出诊断] 收到 applicationShouldTerminate：当前事件={event}；Apple Event={apple_event}\n调用栈：\n{}",
        std::backtrace::Backtrace::force_capture()
    );
}

fn shared_application() -> Result<*mut Object, TerminationError> {
    let application: *mut Object =
        unsafe { objc::msg_send![objc::class!(NSApplication), sharedApplication] };
    if application.is_null() {
        Err(TerminationError::MissingApplication)
    } else {
        Ok(application)
    }
}

fn ensure_main_thread() -> Result<(), TerminationError> {
    let is_main: BOOL = unsafe { objc::msg_send![objc::class!(NSThread), isMainThread] };
    if is_main == YES {
        Ok(())
    } else {
        Err(TerminationError::NotMainThread)
    }
}
