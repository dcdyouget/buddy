use super::*;

fn asset(url: &str) -> serde_json::Value {
    serde_json::json!({
        "url": url,
        "size": 10,
        "sha256": "a".repeat(64),
        "signature": "c2ln",
    })
}

fn manifest_json(version: &str) -> String {
    serde_json::json!({
        "schema": 1,
        "version": version,
        "pub_date": "2026-10-04T00:00:00Z",
        "notes": "修复问题\n新增功能",
        "platforms": {
            "darwin-aarch64": {
                "update": asset("https://example.com/Buddy.app.tar.xz"),
                "installer": asset("https://example.com/Buddy.dmg"),
            }
        }
    })
    .to_string()
}

#[test]
fn newer_version_is_selected_with_notes() {
    let manifest = Manifest::parse(&manifest_json("0.2.0")).unwrap();
    let release = select_release(&manifest, "0.1.0", "darwin-aarch64")
        .unwrap()
        .unwrap();
    assert_eq!(release.version, "0.2.0");
    assert_eq!(release.notes, "修复问题\n新增功能");
    assert!(release.update.url.ends_with(".app.tar.xz"));
}

#[test]
fn windows_release_is_selected_from_the_same_manifest() {
    let mut value: serde_json::Value = serde_json::from_str(&manifest_json("0.2.0")).unwrap();
    value["platforms"]["windows-x86_64"] = serde_json::json!({
        "update": asset("https://example.com/Buddy_0.2.0_x86_64.exe"),
        "installer": asset("https://example.com/Buddy_0.2.0_x86_64.zip"),
    });
    let manifest = Manifest::parse(&value.to_string()).unwrap();

    let release = select_release(&manifest, "0.1.0", "windows-x86_64")
        .unwrap()
        .unwrap();
    assert!(release.update.url.ends_with("Buddy_0.2.0_x86_64.exe"));
}

#[test]
fn same_or_older_version_is_latest() {
    let manifest = Manifest::parse(&manifest_json("0.1.0")).unwrap();
    assert!(select_release(&manifest, "0.1.0", "darwin-aarch64").unwrap().is_none());
    assert!(select_release(&manifest, "0.2.0", "darwin-aarch64").unwrap().is_none());
}

#[test]
fn semver_ordering_is_numeric_not_lexical() {
    let manifest = Manifest::parse(&manifest_json("0.10.0")).unwrap();
    assert!(select_release(&manifest, "0.9.0", "darwin-aarch64").unwrap().is_some());
}

#[test]
fn missing_platform_is_latest_not_error() {
    let manifest = Manifest::parse(&manifest_json("0.2.0")).unwrap();
    assert!(select_release(&manifest, "0.1.0", "windows-x86_64").unwrap().is_none());
}

#[test]
fn invalid_manifests_are_rejected() {
    let bad = [
        "not json".to_string(),
        manifest_json("abc"),
        manifest_json("0.2.0").replace("\"schema\":1", "\"schema\":2"),
        manifest_json("0.2.0").replace("https://example.com/Buddy.dmg", "http://example.com/a"),
        manifest_json("0.2.0").replace(&"a".repeat(64), "xyz"),
        manifest_json("0.2.0").replace("\"size\":10", "\"size\":0"),
        manifest_json("0.2.0").replace("c2ln", " "),
    ];
    for text in bad {
        assert!(
            matches!(Manifest::parse(&text), Err(UpdateError::Manifest(_))),
            "应拒绝：{text}"
        );
    }
}
