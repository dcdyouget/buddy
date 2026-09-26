use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Each writer owns its temporary file, including across app processes.
pub(super) fn write_file_atomic(
    dir: &Path,
    file_name: &str,
    content: &str,
    label: &str,
) -> Result<(), String> {
    let (temporary_path, mut file) = loop {
        let sequence = WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(
            ".{file_name}.{}.{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => break (path, file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("创建{label}临时文件失败: {error}")),
        }
    };

    let result = (|| {
        file.write_all(content.as_bytes())
            .map_err(|error| format!("写入{label}临时文件失败: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("同步{label}临时文件失败: {error}"))?;
        drop(file);
        fs::rename(&temporary_path, dir.join(file_name))
            .map_err(|error| format!("写入{label}失败: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn independent_writers_do_not_share_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(16));
        let handles = (0..16)
            .map(|index| {
                let dir = directory.path().to_path_buf();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let content =
                        serde_json::json!({ "writer": index, "payload": "x".repeat(4096) });
                    barrier.wait();
                    write_file_atomic(&dir, "config.json", &content.to_string(), "配置")
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let stored: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(directory.path().join("config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(stored["payload"].as_str().unwrap().len(), 4096);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replace_removes_only_its_own_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("config.json")).unwrap();
        assert!(write_file_atomic(directory.path(), "config.json", "{}", "配置").is_err());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert!(directory.path().join("config.json").is_dir());
    }
}
