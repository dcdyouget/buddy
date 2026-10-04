//! 流式下载更新包，边下边算 sha256；全部校验通过才交给安装。

use crate::{Asset, Downloaded, PUBLIC_KEY, Release, UpdateError, cache_dir, verify};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// 下载 `release` 的更新包到缓存目录并校验；`progress(已下载, 总大小)` 在每个数据块后回调。
pub async fn download(
    client: &reqwest::Client,
    release: &Release,
    mut progress: impl FnMut(u64, u64) + Send,
) -> Result<Downloaded, UpdateError> {
    let root = cache_dir()?;
    // 只保留本次下载：上次失败或被放弃的残留一律清掉
    if root.exists() {
        tokio::fs::remove_dir_all(&root)
            .await
            .map_err(|e| UpdateError::Install(format!("清理更新缓存失败：{e}")))?;
    }
    let dir = root.join(&release.version);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| UpdateError::Install(format!("创建更新缓存失败：{e}")))?;
    let asset = &release.update;
    let path = dir.join(archive_name(&asset.url));

    let mut response = client
        .get(&asset.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|e| UpdateError::Install(format!("写入更新包失败：{e}")))?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    progress(0, asset.size);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| UpdateError::Network(e.to_string()))?
    {
        received += chunk.len() as u64;
        if received > asset.size {
            return Err(UpdateError::Integrity(format!(
                "实际大小超过清单声明的 {} 字节",
                asset.size
            )));
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| UpdateError::Install(format!("写入更新包失败：{e}")))?;
        progress(received, asset.size);
    }
    file.flush()
        .await
        .map_err(|e| UpdateError::Install(format!("写入更新包失败：{e}")))?;
    drop(file);

    check_digest(received, &hex::encode(hasher.finalize()), asset)?;
    verify_file(&path, asset, PUBLIC_KEY).await?;
    Ok(Downloaded {
        version: release.version.clone(),
        archive: path,
    })
}

/// 大小与 sha256 必须与清单完全一致。
fn check_digest(received: u64, sha256: &str, asset: &Asset) -> Result<(), UpdateError> {
    if received != asset.size {
        return Err(UpdateError::Integrity(format!(
            "大小 {received} 与清单 {} 不一致",
            asset.size
        )));
    }
    if !sha256.eq_ignore_ascii_case(&asset.sha256) {
        return Err(UpdateError::Integrity(format!(
            "sha256 {sha256} 与清单 {} 不一致",
            asset.sha256
        )));
    }
    Ok(())
}

/// 对落盘文件做签名校验（读取的是已落盘内容，而不是网络流）。
async fn verify_file(path: &Path, asset: &Asset, key: &str) -> Result<(), UpdateError> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| UpdateError::Install(format!("读取更新包失败：{e}")))?;
    verify::verify_signature(&data, &asset.signature, key)
}

/// 取 URL 最后一段作为文件名；含异常字符时回退为固定名称。
fn archive_name(url: &str) -> PathBuf {
    let last = url.rsplit('/').next().unwrap_or_default();
    let safe = !last.is_empty()
        && last
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    PathBuf::from(if safe { last } else { "update.tar.gz" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(size: u64, sha256: &str) -> Asset {
        Asset {
            url: "https://example.com/a".into(),
            size,
            sha256: sha256.into(),
            signature: "x".into(),
        }
    }

    #[test]
    fn digest_must_match_size_and_hash() {
        let hash = hex::encode(Sha256::digest(b"abc"));
        assert!(check_digest(3, &hash, &asset(3, &hash.to_uppercase())).is_ok());
        assert!(matches!(
            check_digest(2, &hash, &asset(3, &hash)),
            Err(UpdateError::Integrity(_))
        ));
        assert!(matches!(
            check_digest(3, &hash, &asset(3, &"0".repeat(64))),
            Err(UpdateError::Integrity(_))
        ));
    }

    #[test]
    fn archive_name_rejects_path_tricks() {
        assert_eq!(
            archive_name("https://x/releases/0.1.0/Buddy_0.1.0_aarch64.app.tar.gz"),
            PathBuf::from("Buddy_0.1.0_aarch64.app.tar.gz")
        );
        assert_eq!(archive_name("https://x/a%2F..%2Fb"), PathBuf::from("update.tar.gz"));
        assert_eq!(archive_name("https://x/"), PathBuf::from("update.tar.gz"));
    }
}
