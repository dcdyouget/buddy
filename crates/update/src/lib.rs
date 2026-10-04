//! Buddy 自更新（Phase 08）。
//!
//! 流程：读取**固定地址**的版本清单 → 选出比当前版本新的本平台更新包 →
//! 流式下载并同时计算 sha256 → 校验大小、sha256 与 minisign 签名 → 替换安装 → 重启。
//!
//! 本 crate 不依赖 GPUI：界面层只调用 [`check`]、[`download`]、[`install`] 和 [`relaunch_after_exit`]。

mod download;
mod error;
#[cfg(target_os = "macos")]
mod macos;
mod manifest;
mod verify;

pub use download::download;
pub use error::UpdateError;
pub use manifest::{Asset, Manifest, PlatformRelease, Release, select_release};

use std::path::{Path, PathBuf};

/// 版本清单的固定地址。**写入已发布客户端后不可再改**：
/// 换地址只能在新版本客户端中进行，并让旧地址继续发布一段时间。
pub const MANIFEST_URL: &str =
    "https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json";

/// 更新包签名公钥（minisign Ed25519，外层 base64，与 v1 `src-tauri/tauri.conf.json` 同一把）。
/// 私钥只在发布机 `~/.tauri/buddy.key`；丢失后已发布客户端无法再验证任何新包。
pub const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDhGQzA1NkE1MEE5NUU2NDMKUldSRDVwVUtwVmJBajQrUldZVjhOZm5ZbUkwdlh4T1h0eWx4Z2dSKzZEZit0andnOXptWERYWkcK";

/// 与 Info.plist `CFBundleIdentifier` 一致；安装前校验新包，避免装入其他应用。
pub const BUNDLE_ID: &str = "com.buddy.chat";

/// 当前运行版本（来自 workspace 版本号，发布脚本保证与 Info.plist 一致）。
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 清单中的平台键；不支持的平台返回 `None`（界面显示「当前平台暂不支持自动更新」）。
pub fn current_platform() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("darwin-aarch64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("windows-x86_64")
    } else {
        None
    }
}

/// 是否以可自更新的安装包形式运行；开发构建与示例程序不做后台检查。
pub fn is_installed_app() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::current_app().is_ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// 读取清单并返回可用更新；已是最新返回 `Ok(None)`。
pub async fn check(client: &reqwest::Client) -> Result<Option<Release>, UpdateError> {
    let platform = current_platform().ok_or(UpdateError::UnsupportedPlatform)?;
    let response = client
        .get(MANIFEST_URL)
        // 清单是发布开关，必须绕过任何中间缓存
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    let text = response
        .text()
        .await
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    let manifest = Manifest::parse(&text)?;
    select_release(&manifest, current_version(), platform)
}

/// 下载暂存目录：`<系统缓存>/com.buddy.chat/updates`。每次下载前整体清空。
pub fn cache_dir() -> Result<PathBuf, UpdateError> {
    dirs::cache_dir()
        .map(|dir| dir.join(BUNDLE_ID).join("updates"))
        .ok_or_else(|| UpdateError::Install("无法定位系统缓存目录".into()))
}

/// 已下载并通过全部校验、等待安装的更新包。
#[derive(Debug, Clone)]
pub struct Downloaded {
    pub version: String,
    pub archive: PathBuf,
}

/// 安装完成、等待当前进程退出后启动的新应用。
#[derive(Debug, Clone)]
pub struct Installed {
    pub app: PathBuf,
}

/// 用已验证的更新包替换当前应用（同步、可能弹出管理员授权，调用方放到后台线程）。
pub fn install(downloaded: &Downloaded) -> Result<Installed, UpdateError> {
    #[cfg(target_os = "macos")]
    {
        macos::install(&downloaded.archive, &downloaded.version)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = downloaded;
        Err(UpdateError::UnsupportedPlatform)
    }
}

/// 派生一个独立进程：等当前进程退出后再打开新应用。
/// 必须等旧进程退出——新进程启动时若旧实例仍在，单实例锁会把它转发回旧实例后直接退出。
pub fn relaunch_after_exit(installed: &Installed) -> Result<(), UpdateError> {
    #[cfg(target_os = "macos")]
    {
        macos::relaunch_after_exit(&installed.app)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = installed;
        Err(UpdateError::UnsupportedPlatform)
    }
}

/// 启动后清理：上次更新留下的旧应用备份与下载缓存。失败只记日志。
pub fn cleanup_after_launch() {
    #[cfg(target_os = "macos")]
    macos::cleanup_backups();
    if let Ok(dir) = cache_dir()
        && dir.exists()
        && let Err(error) = std::fs::remove_dir_all(&dir)
    {
        log::warn!("清理更新缓存失败：{error}");
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
