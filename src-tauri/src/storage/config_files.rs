use crate::models::{normalize_model_ids, AppConfig};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

// Full-file writes must not race over the same temporary path.
static CONFIG_WRITE_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn read_config(dir: &PathBuf) -> Result<AppConfig, String> {
    let path = dir.join("config.json");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let content = fs::read_to_string(path).map_err(|error| format!("读取配置文件失败: {error}"))?;
    let mut config = serde_json::from_str(&content).unwrap_or_else(|error| {
        log::warn!("配置文件损坏，使用默认配置: {error}");
        AppConfig::default()
    });
    normalize_model_ids(&mut config);
    Ok(config)
}

pub(super) fn save_config(dir: &PathBuf, config: &AppConfig) -> Result<(), String> {
    let _guard = CONFIG_WRITE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    fs::create_dir_all(dir).map_err(|error| format!("无法创建数据目录: {error}"))?;
    let mut config = config.clone();
    normalize_model_ids(&mut config);
    let content = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("序列化配置失败: {error}"))?;
    super::write_file_atomic(dir, "config.json", &content, "配置文件")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn legacy_config_migrates_on_read_without_rewriting_or_losing_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let legacy = serde_json::json!({
            "providers": [{
                "id": "provider", "name": "服务", "base_url": "https://example.invalid",
                "api_key": "test-only-key", "enabled_model_ids": ["model"]
            }],
            "models": [{
                "id": "model", "provider_id": "provider", "display_name": "模型",
                "context_window": 32000, "latency_ms": null, "supports_vision": true
            }],
            "selected_model_id": "model"
        })
        .to_string();
        let dir = directory.path().to_path_buf();
        fs::write(dir.join("config.json"), &legacy).unwrap();

        let migrated = read_config(&dir).unwrap();
        assert_eq!(migrated.selected_model_id, "provider::model");
        assert_eq!(migrated.providers[0].enabled_model_ids, ["provider::model"]);
        assert_eq!(migrated.providers[0].api_key, "test-only-key");
        assert!(migrated.models[0].supports_vision);
        assert_eq!(fs::read_to_string(dir.join("config.json")).unwrap(), legacy);

        save_config(&dir, &migrated).unwrap();
        assert_eq!(
            serde_json::to_value(read_config(&dir).unwrap()).unwrap(),
            serde_json::to_value(&migrated).unwrap()
        );
    }

    #[test]
    fn concurrent_config_writes_all_succeed_and_leave_complete_json() {
        let directory = tempfile::tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(16));
        let handles = (0..16)
            .map(|index| {
                let dir = directory.path().to_path_buf();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let config = AppConfig {
                        hotkey: format!("review-{index}"),
                        ..AppConfig::default()
                    };
                    barrier.wait();
                    save_config(&dir, &config)
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let config = read_config(&directory.path().to_path_buf()).unwrap();
        assert!(config.hotkey.starts_with("review-"));
        assert!(!directory.path().join(".config.json.tmp").exists());
    }
}
