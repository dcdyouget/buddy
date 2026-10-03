//! Cross-process single-instance ownership for the desktop shell.
//!
//! The owner is protected by a per-user `flock` file.  A Unix-domain socket is
//! only the wake-up transport; it is never used as the ownership primitive.
//! This ordering lets a crashed owner leave a stale socket without allowing a
//! live secondary process to unlink an active owner's endpoint.

#![cfg(unix)]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

const LOCK_MODE: u32 = 0o600;
const SOCKET_MODE: u32 = 0o600;
const DIRECTORY_MODE: u32 = 0o700;
const CONNECT_RETRIES: usize = 40;
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(25);
const BIND_RETRIES: usize = 20;
const BIND_RETRY_DELAY: Duration = Duration::from_millis(25);
const CLIENT_TIMEOUT: Duration = Duration::from_millis(750);

/// A wake request received from a secondary process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wake;

/// The result of attempting to acquire the process owner.
pub enum Acquire {
    /// This process owns the instance and must retain the guard for its lifetime.
    Owner {
        /// The file lock and listener lifetime guard.
        guard: InstanceGuard,
        /// Requests forwarded by secondary processes.
        receiver: UnboundedReceiver<Wake>,
    },
    /// A live owner acknowledged this process's wake request.
    Forwarded,
}

/// Errors from single-instance acquisition or wake forwarding.
#[derive(Debug)]
pub enum InstanceError {
    /// A filesystem operation failed.
    Io {
        /// 失败操作的中文名称。
        operation: &'static str,
        /// 原始系统错误。
        source: io::Error,
    },
    /// The lock is held but its owner could not be reached in the bounded window.
    OwnerUnreachable,
    /// A connected endpoint did not speak the instance protocol.
    Protocol(String),
    /// The requested directory is not private to the current user.
    InsecureDirectory(PathBuf),
    /// This target is not supported on the current platform.
    Unsupported,
}

impl std::fmt::Display for InstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { operation, source } => write!(f, "{operation}失败：{source}"),
            Self::OwnerUnreachable => write!(f, "已有实例占用锁，但在限定时间内无法转发唤起"),
            Self::Protocol(message) => write!(f, "单实例协议错误：{message}"),
            Self::InsecureDirectory(path) => {
                write!(
                    f,
                    "单实例目录不是用户私有目录（需要 0700）：{}",
                    path.display()
                )
            }
            Self::Unsupported => write!(f, "当前平台不支持单实例 Unix socket"),
        }
    }
}

impl std::error::Error for InstanceError {}

/// The owner lifetime guard.
pub struct InstanceGuard {
    lock: FileLock,
    socket_path: PathBuf,
    stop: Arc<AtomicBool>,
    listener_thread: Option<JoinHandle<()>>,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        self.stop_endpoint();
        // Keep the lock field live until after socket cleanup.
        let _ = &self.lock;
    }
}

impl InstanceGuard {
    /// Stop accepting wake requests and clean the socket before process exit.
    ///
    /// The flock descriptor is intentionally kept alive by forgetting the
    /// guard. The operating system releases it only when the process exits,
    /// which prevents residual asynchronous tasks from racing a replacement
    /// owner during shutdown.
    pub fn prepare_process_exit(mut self) {
        self.stop_endpoint();
        std::mem::forget(self);
    }

    fn stop_endpoint(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake the non-blocking accept loop so normal shutdown does not wait
        // for its polling interval.  The connection is deliberately ignored.
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(thread) = self.listener_thread.take() {
            let _ = thread.join();
        }
        // Only the owner ever removes the socket.  The lock file is retained
        // so a replacement owner cannot race on a newly-created inode.
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// Acquire the owner or forward a wake request to the existing owner.
///
/// `socket_path` must be inside a user-private (0700) directory.  The sibling
/// `.lock` file is created with mode 0600 and is never removed.
pub fn acquire<P: AsRef<Path>>(socket_path: P) -> Result<Acquire, InstanceError> {
    let socket_path = socket_path.as_ref().to_path_buf();
    let parent = socket_path.parent().ok_or_else(|| InstanceError::Io {
        operation: "解析单实例目录",
        source: io::Error::new(io::ErrorKind::InvalidInput, "socket 路径没有父目录"),
    })?;
    ensure_private_directory(parent)?;
    let lock_path = lock_path(&socket_path);
    let lock = open_lock(&lock_path)?;

    match try_lock(&lock)? {
        LockAttempt::Acquired => become_owner(lock, socket_path),
        LockAttempt::Busy => {
            drop(lock);
            match forward_with_retry(&socket_path)? {
                ForwardAttempt::Forwarded => Ok(Acquire::Forwarded),
                ForwardAttempt::NoEndpoint => {
                    // The original owner may have crashed after releasing
                    // flock but before unlinking its socket. Reopen the lock;
                    // only its successful acquisition permits stale cleanup.
                    let replacement = open_lock(&lock_path)?;
                    match try_lock(&replacement)? {
                        LockAttempt::Acquired => become_owner(replacement, socket_path),
                        LockAttempt::Busy => Err(InstanceError::OwnerUnreachable),
                    }
                }
            }
        }
    }
}

fn become_owner(lock: FileLock, socket_path: PathBuf) -> Result<Acquire, InstanceError> {
    let listener = bind_listener(&socket_path)?;
    let (sender, receiver) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let listener_thread = thread::Builder::new()
        .name("buddy-instance-listener".to_owned())
        .spawn(move || listen(listener, thread_stop, sender))
        .map_err(|source| InstanceError::Io {
            operation: "启动单实例监听线程",
            source,
        });
    let listener_thread = match listener_thread {
        Ok(thread) => thread,
        Err(error) => {
            let _ = fs::remove_file(&socket_path);
            return Err(error);
        }
    };

    Ok(Acquire::Owner {
        guard: InstanceGuard {
            lock,
            socket_path,
            stop,
            listener_thread: Some(listener_thread),
        },
        receiver,
    })
}

fn ensure_private_directory(path: &Path) -> Result<(), InstanceError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(DIRECTORY_MODE)
                .create(path)
                .map_err(|source| InstanceError::Io {
                    operation: "创建单实例目录",
                    source,
                })?;
        }
        Err(source) => {
            return Err(InstanceError::Io {
                operation: "读取单实例目录",
                source,
            });
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| InstanceError::Io {
        operation: "读取单实例目录权限",
        source,
    })?;
    let current_uid = unsafe { libc::getuid() };
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != current_uid
        || metadata.permissions().mode() & 0o777 != DIRECTORY_MODE
    {
        return Err(InstanceError::InsecureDirectory(path.to_path_buf()));
    }
    Ok(())
}

fn lock_path(socket_path: &Path) -> PathBuf {
    socket_path.with_extension("lock")
}

fn open_lock(path: &Path) -> Result<FileLock, InstanceError> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(LOCK_MODE)
        .open(path)
        .map_err(|source| InstanceError::Io {
            operation: "打开单实例锁文件",
            source,
        })?;
    file.set_permissions(fs::Permissions::from_mode(LOCK_MODE))
        .map_err(|source| InstanceError::Io {
            operation: "设置单实例锁文件权限",
            source,
        })?;
    Ok(FileLock { file })
}

enum LockAttempt {
    Acquired,
    Busy,
}

fn try_lock(lock: &FileLock) -> Result<LockAttempt, InstanceError> {
    // SAFETY: the descriptor belongs to the live File held by FileLock.
    let result = unsafe { libc::flock(lock.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(LockAttempt::Acquired);
    }
    let source = io::Error::last_os_error();
    if matches!(
        source.raw_os_error(),
        Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN
    ) {
        Ok(LockAttempt::Busy)
    } else {
        Err(InstanceError::Io {
            operation: "占用单实例锁",
            source,
        })
    }
}

struct FileLock {
    file: File,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // SAFETY: the descriptor belongs to the live File held by FileLock.
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

fn bind_listener(path: &Path) -> Result<UnixListener, InstanceError> {
    for attempt in 0..BIND_RETRIES {
        match UnixListener::bind(path) {
            Ok(listener) => {
                if let Err(source) = listener.set_nonblocking(true) {
                    let _ = fs::remove_file(path);
                    return Err(InstanceError::Io {
                        operation: "设置单实例 socket 非阻塞",
                        source,
                    });
                }
                if let Err(source) =
                    fs::set_permissions(path, fs::Permissions::from_mode(SOCKET_MODE))
                {
                    let _ = fs::remove_file(path);
                    return Err(InstanceError::Io {
                        operation: "设置单实例 socket 权限",
                        source,
                    });
                }
                return Ok(listener);
            }
            Err(source)
                if source.kind() == io::ErrorKind::AddrInUse && attempt + 1 < BIND_RETRIES =>
            {
                // The owner lock is already held, so this path can only be a
                // stale endpoint left by a crashed owner of this protocol.
                let _ = fs::remove_file(path);
                thread::sleep(BIND_RETRY_DELAY);
            }
            Err(source) => {
                return Err(InstanceError::Io {
                    operation: "绑定单实例 socket",
                    source,
                });
            }
        }
    }
    unreachable!("the bind retry loop returns on its final iteration")
}

enum ForwardAttempt {
    Forwarded,
    NoEndpoint,
}

fn forward_with_retry(path: &Path) -> Result<ForwardAttempt, InstanceError> {
    for attempt in 0..CONNECT_RETRIES {
        match UnixStream::connect(path) {
            Ok(mut stream) => {
                stream
                    .set_read_timeout(Some(CLIENT_TIMEOUT))
                    .map_err(|source| InstanceError::Io {
                        operation: "设置唤起确认超时",
                        source,
                    })?;
                stream
                    .set_write_timeout(Some(CLIENT_TIMEOUT))
                    .map_err(|source| InstanceError::Io {
                        operation: "设置唤起发送超时",
                        source,
                    })?;
                stream
                    .write_all(b"Wake\n")
                    .map_err(|source| InstanceError::Io {
                        operation: "发送单实例唤起",
                        source,
                    })?;
                let mut reply = [0_u8; 4];
                stream
                    .read_exact(&mut reply)
                    .map_err(|source| InstanceError::Io {
                        operation: "读取单实例唤起确认",
                        source,
                    })?;
                if &reply == b"ACK\n" {
                    return Ok(ForwardAttempt::Forwarded);
                }
                return Err(InstanceError::Protocol(format!(
                    "收到未知确认 {:?}",
                    String::from_utf8_lossy(&reply)
                )));
            }
            Err(source)
                if matches!(
                    source.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) && attempt + 1 < CONNECT_RETRIES =>
            {
                thread::sleep(CONNECT_RETRY_DELAY);
            }
            Err(source)
                if matches!(
                    source.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                let _ = source;
                return Ok(ForwardAttempt::NoEndpoint);
            }
            Err(source) => {
                return Err(InstanceError::Io {
                    operation: "连接已有实例",
                    source,
                });
            }
        }
    }
    Ok(ForwardAttempt::NoEndpoint)
}

fn listen(listener: UnixListener, stop: Arc<AtomicBool>, sender: UnboundedSender<Wake>) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => handle_client(stream, &sender),
            Err(source) if source.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(CONNECT_RETRY_DELAY);
            }
            Err(_) => {
                if !stop.load(Ordering::Acquire) {
                    thread::sleep(CONNECT_RETRY_DELAY);
                }
            }
        }
    }
}

fn handle_client(mut stream: UnixStream, sender: &UnboundedSender<Wake>) {
    let _ = stream.set_read_timeout(Some(CLIENT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_TIMEOUT));
    let mut request = [0_u8; 5];
    let read = stream.read_exact(&mut request);
    if read.is_ok() && request == *b"Wake\n" && sender.send(Wake).is_ok() {
        let _ = stream.write_all(b"ACK\n");
    } else {
        let _ = stream.write_all(b"ERR\n");
    }
}
