use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn release(version: &str) -> Value {
    let name = format!("Gavnel-Setup-{version}-x64.exe");
    json!({ "tag_name": format!("v{version}"), "draft": false, "prerelease": false,
        "assets": [{ "name": name, "size": 100, "state": "uploaded", "digest": format!("sha256:{}", "a".repeat(64)),
        "browser_download_url": format!("{REPOSITORY}/releases/download/v{version}/{name}") }] })
}
fn parse(value: &Value) -> Result<Option<Release>> {
    metadata::parse(&serde_json::to_vec(value)?, &Version::parse("0.3.0")?)
}

#[test]
fn release_versions_compare_semantically_and_only_stable_newer_builds_are_offered() {
    assert_eq!(
        parse(&release("0.10.0")).unwrap().unwrap().version,
        Version::parse("0.10.0").unwrap()
    );
    for version in ["0.2.9", "0.3.0", "0.4.0-beta.1"] {
        assert!(parse(&release(version)).unwrap().is_none());
    }
    for field in ["draft", "prerelease"] {
        let mut value = release("0.4.0");
        value[field] = json!(true);
        assert!(parse(&value).unwrap().is_none());
    }
}

#[test]
fn releases_require_the_expected_repo_asset_and_github_digest() {
    for (field, invalid) in [
        (
            "browser_download_url",
            json!("https://example.com/Gavnel.exe"),
        ),
        (
            "browser_download_url",
            json!(format!(
                "{REPOSITORY}/releases/download/v0.4.0/../malware.exe"
            )),
        ),
        ("state", json!("new")),
        ("size", json!(0)),
        ("size", json!(MAX_INSTALLER + 1)),
        ("digest", Value::Null),
        ("digest", json!("sha256:bad")),
        ("digest", json!(format!("sha256:{}", "z".repeat(64)))),
        ("name", json!("Gavnel-Setup-0.4.0-arm64.exe")),
    ] {
        let mut value = release("0.4.0");
        value["assets"][0][field] = invalid;
        assert!(parse(&value).is_err(), "Must reject invalid {field}");
    }
}

#[test]
fn downloaded_installer_is_verified_before_execution_and_tampering_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("setup.exe");
    let bytes = b"MZtest installer fixture";
    std::fs::write(&path, bytes).unwrap();
    let hash = format!("{:x}", Sha256::digest(bytes));
    assert!(download::verify(&path, &hash, bytes.len() as u64).is_ok());
    std::fs::write(&path, b"MZchanged installer data").unwrap();
    assert!(download::verify(&path, &hash, bytes.len() as u64).is_err());
    std::fs::write(&path, bytes).unwrap();
    assert!(download::verify(&path, &hash, bytes.len() as u64 + 1).is_err());
    let html = b"<html>not an installer</html>";
    std::fs::write(&path, html).unwrap();
    assert!(
        download::verify(
            &path,
            &format!("{:x}", Sha256::digest(html)),
            html.len() as u64
        )
        .is_err()
    );
}

#[test]
fn only_installed_releases_can_replace_app_files() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("Gavnel.exe");
    std::fs::write(&exe, b"fixture").unwrap();
    assert!(!managed_executable(&exe));
    std::fs::write(dir.path().join("gavnel-install.txt"), INSTALL_MARKER).unwrap();
    assert!(!managed_executable(&exe));
    std::fs::write(dir.path().join("unins000.exe"), b"fixture").unwrap();
    assert!(managed_executable(&exe));
    assert!(!managed_executable(&dir.path().join("different.exe")));
}

#[test]
fn obsolete_update_preference_is_ignored() {
    let old: crate::config::Settings = serde_json::from_str("{}").unwrap();
    let restored: crate::config::Settings =
        serde_json::from_str(r#"{"check_for_updates":false}"#).unwrap();
    assert_eq!(old, restored);
    assert!(
        serde_json::to_value(&restored)
            .unwrap()
            .get("check_for_updates")
            .is_none()
    );
}
