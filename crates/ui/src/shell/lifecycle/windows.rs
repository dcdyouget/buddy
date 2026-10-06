//! Windows single-instance ownership and wake-up transport.
//!
//! The mutex is the ownership primitive. An auto-reset named event lets a
//! second launch ask the owner to show its existing window without starting a
//! second engine or tray icon.

use crate::shell::{AppShell, runtime};
use gpui::{AsyncApp, Global, Subscription, Task, WindowHandle};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
#[cfg(test)]
use windows_sys::Win32::System::Threading::{MUTEX_MODIFY_STATE, OpenMutexW};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0},
    System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent,
        WaitForSingleObject,
    },
};

const MUTEX_NAME: &str = "Local\\com.buddy.chat.v2.instance";
const WAKE_EVENT_NAME: &str = "Local\\com.buddy.chat.v2.wake";

/// A wake-up request received from a later Buddy launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wake;

/// Outcome of attempting to become the Windows Buddy instance owner.
pub enum Acquire {
    /// This process owns the mutex and receives future wake requests.
    Owner {
        /// Keeps the named mutex, event, and listener alive.
        guard: InstanceGuard,
        /// Wake requests from subsequently launched processes.
        receiver: UnboundedReceiver<Wake>,
    },
    /// An existing owner was signalled successfully.
    Forwarded,
}

/// Lifetime guard for the Windows mutex and its wake listener.
pub struct InstanceGuard {
    mutex: HANDLE,
    event: HANDLE,
    listener_running: Arc<AtomicBool>,
    listener: Option<JoinHandle<()>>,
}

type ListenerSpawner =
    fn(HANDLE, UnboundedSender<Wake>, Arc<AtomicBool>) -> Result<JoinHandle<()>, String>;

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        self.listener_running.store(false, Ordering::Release);
        unsafe {
            if !self.event.is_null() {
                // Wake the listener so it observes the stop flag before this handle closes.
                SetEvent(self.event);
            }
        }
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
        unsafe {
            if !self.event.is_null() {
                CloseHandle(self.event);
            }
            if !self.mutex.is_null() {
                CloseHandle(self.mutex);
            }
        }
    }
}

/// Acquire the per-user Windows instance lock or signal its current owner.
pub fn acquire() -> Result<Acquire, String> {
    acquire_named(MUTEX_NAME, WAKE_EVENT_NAME, spawn_wake_listener)
}

fn acquire_named(
    mutex_name: &str,
    event_name: &str,
    listener_spawner: ListenerSpawner,
) -> Result<Acquire, String> {
    let event_name = wide(event_name);
    let mutex_name = wide(mutex_name);
    unsafe {
        // Create the event first so a racing secondary launch can always signal it.
        let event = CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr());
        if event.is_null() {
            return Err(last_error("创建单实例唤醒事件失败"));
        }
        let mutex = CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
        if mutex.is_null() {
            let error = last_error("创建单实例锁失败");
            CloseHandle(event);
            return Err(error);
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let wake_event = OpenEventW(EVENT_MODIFY_STATE, 0, event_name.as_ptr());
            let signalled = !wake_event.is_null() && SetEvent(wake_event) != 0;
            let error = (!signalled).then(|| last_error("向已运行的 Buddy 发送唤醒请求失败"));
            if !wake_event.is_null() {
                CloseHandle(wake_event);
            }
            CloseHandle(mutex);
            CloseHandle(event);
            if signalled {
                return Ok(Acquire::Forwarded);
            }
            return Err(error.expect("a failed wake must capture its Windows error"));
        }

        let (sender, receiver) = mpsc::unbounded_channel();
        let listener_running = Arc::new(AtomicBool::new(true));
        let listener = match listener_spawner(event, sender, listener_running.clone()) {
            Ok(listener) => listener,
            Err(error) => {
                CloseHandle(mutex);
                CloseHandle(event);
                return Err(error);
            }
        };
        Ok(Acquire::Owner {
            guard: InstanceGuard {
                mutex,
                event,
                listener_running,
                listener: Some(listener),
            },
            receiver,
        })
    }
}

fn spawn_wake_listener(
    event: HANDLE,
    sender: UnboundedSender<Wake>,
    running: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    let event = event as usize;
    std::thread::Builder::new()
        .name("buddy-instance-wake".into())
        .spawn(move || {
            loop {
                let result = unsafe { WaitForSingleObject(event as HANDLE, INFINITE) };
                if result != WAIT_OBJECT_0
                    || !running.load(Ordering::Acquire)
                    || sender.send(Wake).is_err()
                {
                    return;
                }
            }
        })
        .map_err(|error| format!("创建 Buddy 单实例唤醒线程失败：{error}"))
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn last_error(operation: &str) -> String {
    format!("{operation}：{}", std::io::Error::last_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    static TEST_NAME_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn test_names() -> (String, String) {
        let sequence = TEST_NAME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let base = format!(
            "Local\\com.buddy.chat.test.{}.{}",
            std::process::id(),
            sequence
        );
        (format!("{base}.mutex"), format!("{base}.wake"))
    }

    #[tokio::test]
    async fn second_launch_forwards_wake_and_drop_allows_a_new_owner() {
        let (mutex_name, event_name) = test_names();
        let first = match acquire_named(&mutex_name, &event_name, spawn_wake_listener) {
            Ok(first) => first,
            Err(error) => panic!("first launch failed: {error}"),
        };
        let (guard, mut receiver) = match first {
            Acquire::Owner { guard, receiver } => (guard, receiver),
            Acquire::Forwarded => panic!("first launch must own its private mutex"),
        };

        let second = match acquire_named(&mutex_name, &event_name, spawn_wake_listener) {
            Ok(second) => second,
            Err(error) => panic!("second launch failed: {error}"),
        };
        assert!(matches!(second, Acquire::Forwarded));
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), receiver.recv())
                .await
                .expect("owner did not receive the forwarded wake"),
            Some(Wake)
        );

        drop(guard);
        let replacement = match acquire_named(&mutex_name, &event_name, spawn_wake_listener) {
            Ok(replacement) => replacement,
            Err(error) => panic!("launch after owner drop failed: {error}"),
        };
        assert!(matches!(replacement, Acquire::Owner { .. }));
        drop(replacement);
    }

    #[test]
    fn listener_spawn_failure_releases_the_private_kernel_objects() {
        let (mutex_name, event_name) = test_names();
        let result = acquire_named(&mutex_name, &event_name, |_event, _sender, _running| {
            Err("injected listener startup failure".to_string())
        });
        assert!(result.is_err());

        let event_name = wide(&event_name);
        let mutex_name = wide(&mutex_name);
        unsafe {
            let event = OpenEventW(EVENT_MODIFY_STATE, 0, event_name.as_ptr());
            assert!(event.is_null(), "failed owner leaked its wake event handle");
            let mutex = OpenMutexW(MUTEX_MODIFY_STATE, 0, mutex_name.as_ptr());
            assert!(mutex.is_null(), "failed owner leaked its mutex handle");
        }
    }
}

struct Lifecycle {
    guard: Option<InstanceGuard>,
    _requests: Task<()>,
    _quit: Subscription,
}

impl Global for Lifecycle {}

/// Keep the owner guard alive and show the retained window for every wake request.
pub fn install(
    handle: WindowHandle<AppShell>,
    guard: InstanceGuard,
    mut requests: UnboundedReceiver<Wake>,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    if cx.update(|cx| cx.has_global::<Lifecycle>()) {
        return Err("Windows 生命周期事件桥已经安装".into());
    }
    let requests = cx.update(|cx| {
        cx.spawn(async move |cx| {
            while requests.recv().await.is_some() {
                if let Err(error) = runtime::show(handle, cx).await {
                    log::error!("唤醒已有 Buddy 窗口失败：{error}");
                }
            }
        })
    });
    let quit = cx.update(|cx| {
        cx.on_app_quit(move |cx| {
            crate::shell::services::shutdown(cx);
            if let Err(error) = runtime::shutdown(cx) {
                log::error!("退出时注销热键失败：{error}");
            }
            let _ = cx.global_mut::<Lifecycle>().guard.take();
            std::future::ready(())
        })
    });
    cx.update(|cx| {
        cx.set_global(Lifecycle {
            guard: Some(guard),
            _requests: requests,
            _quit: quit,
        })
    });
    Ok(())
}
