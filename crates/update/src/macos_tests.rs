use super::*;
use std::fs;

/// 构造一个最小的 ad-hoc 签名 `.app`（与发布脚本的组装方式一致）。
fn fake_app(dir: &Path, name: &str, id: &str, version: &str, marker: &str) -> PathBuf {
    let app = dir.join(name);
    let macos = app.join("Contents/MacOS");
    fs::create_dir_all(&macos).unwrap();
    fs::write(
        app.join("Contents/Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>buddy</string>
<key>CFBundleIdentifier</key><string>{id}</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
</dict></plist>"#
        ),
    )
    .unwrap();
    fs::copy("/usr/bin/true", macos.join("buddy")).unwrap();
    fs::write(macos.join("marker"), marker).unwrap();
    let status = Command::new("/usr/bin/codesign")
        .args(["--force", "--deep", "-s", "-"])
        .arg(&app)
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    app
}

#[test]
fn valid_bundle_passes() {
    let dir = tempfile::tempdir().unwrap();
    let app = fake_app(dir.path(), "Buddy.app", BUNDLE_ID, "0.2.0", "new");
    validate_bundle(&app, "0.2.0").unwrap();
}

#[test]
fn wrong_identity_version_or_tampering_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let other = fake_app(dir.path(), "Other.app", "com.example.other", "0.2.0", "x");
    assert!(validate_bundle(&other, "0.2.0").is_err());

    let app = fake_app(dir.path(), "Buddy.app", BUNDLE_ID, "0.2.0", "new");
    assert!(validate_bundle(&app, "0.3.0").is_err());

    // 签名后改动内容：codesign --strict 必须拒绝
    fs::write(app.join("Contents/MacOS/marker"), "tampered").unwrap();
    assert!(validate_bundle(&app, "0.2.0").is_err());
}

#[test]
fn swap_replaces_target_and_removes_backup() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("Applications");
    let staging = dir.path().join("staging");
    fs::create_dir_all(&installed).unwrap();
    fs::create_dir_all(&staging).unwrap();
    let target = fake_app(&installed, "Buddy.app", BUNDLE_ID, "0.1.0", "old");
    let new_app = fake_app(&staging, "Buddy.app", BUNDLE_ID, "0.2.0", "new");

    swap(&new_app, &target).unwrap();

    let marker = fs::read_to_string(target.join("Contents/MacOS/marker")).unwrap();
    assert_eq!(marker, "new");
    validate_bundle(&target, "0.2.0").unwrap();
    let leftovers: Vec<_> = fs::read_dir(&installed).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(leftovers, vec![std::ffi::OsString::from("Buddy.app")]);
}

#[test]
fn single_app_requires_exactly_one_bundle() {
    let dir = tempfile::tempdir().unwrap();
    assert!(single_app(dir.path()).is_err());
    fake_app(dir.path(), "Buddy.app", BUNDLE_ID, "0.2.0", "a");
    assert!(single_app(dir.path()).is_ok());
    fake_app(dir.path(), "Second.app", BUNDLE_ID, "0.2.0", "b");
    assert!(single_app(dir.path()).is_err());
}

#[test]
fn shell_quote_survives_single_quotes() {
    assert_eq!(shell_quote(Path::new("/a/it's.app")), r"'/a/it'\''s.app'");
}

#[test]
fn development_binary_is_not_an_app() {
    // cargo test 的可执行文件不在 .app 内
    assert!(matches!(current_app(), Err(UpdateError::Install(_))));
}
