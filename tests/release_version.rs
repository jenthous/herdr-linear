//! scripts/check-release-version.sh: 릴리스 태그가 Cargo.toml과 herdr-plugin.toml의 버전과 같아야 한다.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

fn check(tag: &str) -> Output {
    Command::new("/bin/bash")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-release-version.sh"))
        .arg(tag)
        .output()
        .unwrap()
}

#[test]
fn cargo_and_plugin_versions_match_the_release_tag() {
    // 두 파일의 버전이 서로 같아야 이 태그가 통과한다
    let out = check(&format!("v{}", env!("CARGO_PKG_VERSION")));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_different_tag_is_rejected() {
    let out = check("v0.0.0");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("does not match"), "{err}");
}
