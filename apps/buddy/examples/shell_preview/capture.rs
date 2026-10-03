//! 可选实际屏幕采样检查点；默认不阻塞自动化测试。
use buddy_ui::gpui::AsyncApp;
use std::{path::PathBuf, time::Duration};

pub(super) async fn checkpoint(variable: &str, phase: &str, cx: &mut AsyncApp) -> bool {
    let Some(dir) = std::env::var_os(variable).map(PathBuf::from) else {
        return true;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }
    let ready = dir.join(format!("{phase}.ready"));
    let proceed = dir.join(format!("{phase}.continue"));
    let _ = std::fs::remove_file(&proceed);
    // Let the compositor finish the native show/resize transaction before the screen capture.
    cx.background_executor()
        .timer(Duration::from_millis(500))
        .await;
    if std::fs::write(&ready, phase).is_err() {
        return false;
    }
    let mut passed = false;
    for _ in 0..4500 {
        if proceed.is_file() {
            passed = true;
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    let _ = std::fs::remove_file(ready);
    let _ = std::fs::remove_file(proceed);
    passed
}
