//! 版本清单（`channels/stable.json`）的格式与选择逻辑。
//!
//! 清单由发布脚本 `scripts/release/manifest.mjs` 生成；产品介绍页也从同一份清单
//! 读取 `platforms.<平台>.installer.url` 作为最新安装包下载地址。

use crate::UpdateError;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 客户端能理解的清单格式版本；格式不兼容时发布方必须提升它。
pub const SCHEMA: u32 = 1;

/// 完整清单。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub pub_date: String,
    #[serde(default)]
    pub notes: String,
    pub platforms: BTreeMap<String, PlatformRelease>,
}

/// 单个平台的两个制品。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformRelease {
    /// 应用内更新包（macOS：`Buddy.app` 的 tar.gz）。
    pub update: Asset,
    /// 新用户手动安装包（macOS：DMG）。
    pub installer: Asset,
}

/// 一个可下载文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// minisign 签名文件全文的 base64（与 Tauri `.sig` 相同的外层编码）。
    pub signature: String,
}

/// 选中的可用更新。
#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub pub_date: String,
    pub update: Asset,
}

impl Manifest {
    /// 解析并校验结构；字段缺失或取值非法都视为清单无效。
    pub fn parse(text: &str) -> Result<Self, UpdateError> {
        let manifest: Self = serde_json::from_str(text)
            .map_err(|e| UpdateError::Manifest(format!("清单 JSON 无效：{e}")))?;
        if manifest.schema != SCHEMA {
            return Err(UpdateError::Manifest(format!(
                "不支持的清单格式 {}",
                manifest.schema
            )));
        }
        Version::parse(&manifest.version)
            .map_err(|e| UpdateError::Manifest(format!("版本号无效：{e}")))?;
        for (platform, release) in &manifest.platforms {
            for asset in [&release.update, &release.installer] {
                asset
                    .validate()
                    .map_err(|e| UpdateError::Manifest(format!("{platform}：{e}")))?;
            }
        }
        Ok(manifest)
    }
}

impl Asset {
    fn validate(&self) -> Result<(), String> {
        if !self.url.starts_with("https://") {
            return Err(format!("下载地址必须是 HTTPS：{}", self.url));
        }
        if self.size == 0 {
            return Err("文件大小为 0".into());
        }
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("sha256 格式无效".into());
        }
        if self.signature.trim().is_empty() {
            return Err("缺少签名".into());
        }
        Ok(())
    }
}

/// 清单版本严格高于当前版本且包含本平台时返回更新；否则为已是最新。
pub fn select_release(
    manifest: &Manifest,
    current: &str,
    platform: &str,
) -> Result<Option<Release>, UpdateError> {
    let current = Version::parse(current)
        .map_err(|e| UpdateError::Manifest(format!("当前版本号无效：{e}")))?;
    let latest = Version::parse(&manifest.version)
        .map_err(|e| UpdateError::Manifest(format!("版本号无效：{e}")))?;
    if latest <= current {
        return Ok(None);
    }
    // 新版本暂未提供本平台时按「已是最新」处理，而不是报错。
    let Some(release) = manifest.platforms.get(platform) else {
        return Ok(None);
    };
    Ok(Some(Release {
        version: manifest.version.clone(),
        notes: manifest.notes.clone(),
        pub_date: manifest.pub_date.clone(),
        update: release.update.clone(),
    }))
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
