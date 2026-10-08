//! scripts/install.sh: 매니페스트 버전의 릴리스에서 미리 빌드한 바이너리를 받아 놓고,
//! 쓸 수 없으면 cargo로 빌드한다. 네트워크 대신 file:// 가짜 릴리스를 쓴다.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TARGET: &str = "x86_64-unknown-linux-musl";

/// 가짜 플러그인 루트: 저장소의 install.sh와 버전만 적은 매니페스트.
/// `min_herdr_version`을 먼저 써서, 스크립트가 `version` 줄만 읽는지 확인한다.
fn plugin_root(dir: &Path, version: &str) -> PathBuf {
    let root = dir.join("plugin");
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/install.sh"),
        root.join("scripts/install.sh"),
    )
    .unwrap();
    fs::write(
        root.join("herdr-plugin.toml"),
        format!("id = \"jh.linear\"\nmin_herdr_version = \"0.9.3\"\nversion = \"{version}\"\n"),
    )
    .unwrap();
    root
}

/// 가짜 릴리스 `<dir>/releases/v<version>/`에 압축과 체크섬을 둔다.
/// 압축 안 파일 이름은 `binary`이고 실행하면 `fake-release`를 출력한다.
/// `checksum`이 None이면 맞는 값을 쓴다.
fn release(dir: &Path, version: &str, binary: &str, checksum: Option<&str>) -> PathBuf {
    let releases = dir.join("releases");
    let assets = releases.join(format!("v{version}"));
    let stage = dir.join(format!("stage-{version}"));
    fs::create_dir_all(&assets).unwrap();
    fs::create_dir_all(&stage).unwrap();
    let file = stage.join(binary);
    fs::write(&file, "#!/bin/sh\necho fake-release\n").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    let archive = assets.join(format!("herdr-linear-{TARGET}.tar.gz"));
    let ok = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&stage)
        .arg(binary)
        .status()
        .unwrap()
        .success();
    assert!(ok, "tar로 가짜 릴리스를 만들지 못했다");
    let sum = checksum.map_or_else(|| sha256(&archive), str::to_string);
    fs::write(
        assets.join(format!("herdr-linear-{TARGET}.sha256")),
        format!("{sum}  herdr-linear-{TARGET}.tar.gz\n"),
    )
    .unwrap();
    releases
}

fn sha256(path: &Path) -> String {
    let out = Command::new("/bin/sh")
        .arg("-c")
        .arg("if command -v sha256sum >/dev/null 2>&1; then sha256sum \"$1\"; else shasum -a 256 \"$1\"; fi")
        .arg("sh")
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

/// 불리면 작업 폴더에 인자를 `cargo-called`로 남기고, `from-source`를 출력하는 바이너리를 만드는 가짜 cargo.
fn fake_cargo(dir: &Path) -> PathBuf {
    let bin = dir.join("fakebin");
    fs::create_dir_all(&bin).unwrap();
    let cargo = bin.join("cargo");
    fs::write(
        &cargo,
        "#!/bin/sh\necho \"$*\" > cargo-called\nmkdir -p target/release\nprintf '#!/bin/sh\\necho from-source\\n' > target/release/herdr-linear\nchmod 755 target/release/herdr-linear\n",
    )
    .unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

/// install.sh를 플러그인 루트가 아닌 `dir`에서 macOS 기본 bash로 돌린다.
/// PATH는 cargo가 없는 시스템 경로이고, `extra_path`가 있으면 앞에 붙인다.
fn run(
    dir: &Path,
    root: &Path,
    releases: &Path,
    extra_path: Option<&Path>,
    env: &[(&str, &str)],
) -> Output {
    let path = match extra_path {
        Some(p) => format!("{}:/usr/bin:/bin", p.display()),
        None => "/usr/bin:/bin".to_string(),
    };
    let mut cmd = Command::new("/bin/bash");
    cmd.arg(root.join("scripts/install.sh"))
        .current_dir(dir)
        .env_clear()
        .env("PATH", path)
        .env("HOME", dir)
        .env(
            "HERDR_LINEAR_RELEASES",
            format!("file://{}", releases.display()),
        )
        .env("HERDR_LINEAR_TARGET", TARGET)
        .env("HERDR_LINEAR_RETRY_SECONDS", "0");
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// 놓인 바이너리를 실행한 출력.
fn installed(root: &Path) -> String {
    let out = Command::new(root.join("target/release/herdr-linear"))
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn installs_the_prebuilt_binary_for_the_manifest_version() {
    let dir = tempfile::tempdir().unwrap();
    let root = plugin_root(dir.path(), "9.9.9");
    let releases = release(dir.path(), "9.9.9", "herdr-linear", None);
    let out = run(dir.path(), &root, &releases, None, &[]);
    assert!(out.status.success(), "{}", text(&out));
    let bin = root.join("target/release/herdr-linear");
    assert_eq!(
        fs::metadata(&bin).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(installed(&root), "fake-release");
    assert!(!root.join("target/release/herdr-linear.new").exists());
    assert!(
        text(&out).contains("herdr-linear: installed the prebuilt binary for v9.9.9"),
        "{}",
        text(&out)
    );
}

#[test]
fn bad_checksum_is_not_installed_and_explains_without_cargo() {
    for bad in [
        "0000000000000000000000000000000000000000000000000000000000000000",
        "",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = plugin_root(dir.path(), "9.9.9");
        let releases = release(dir.path(), "9.9.9", "herdr-linear", Some(bad));
        let out = run(dir.path(), &root, &releases, None, &[]);
        let t = text(&out);
        assert!(!out.status.success(), "{bad:?}: {t}");
        assert!(
            !root.join("target/release/herdr-linear").exists(),
            "{bad:?}"
        );
        assert!(t.contains("checksum mismatch"), "{t}");
        assert!(t.contains("https://rustup.rs"), "{t}");
    }
}

#[test]
fn falls_back_to_cargo_when_this_version_has_no_release() {
    let dir = tempfile::tempdir().unwrap();
    let root = plugin_root(dir.path(), "9.9.9");
    // 다른 버전의 릴리스만 있다
    let releases = release(dir.path(), "1.0.0", "herdr-linear", None);
    let cargo = fake_cargo(dir.path());
    let out = run(dir.path(), &root, &releases, Some(&cargo), &[]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("could not download"), "{}", text(&out));
    let called = fs::read_to_string(root.join("cargo-called")).unwrap();
    assert_eq!(called.trim(), "build --release", "플러그인 루트에서 불린다");
    assert_eq!(installed(&root), "from-source");
}

#[test]
fn build_from_source_flag_skips_the_download() {
    let dir = tempfile::tempdir().unwrap();
    let root = plugin_root(dir.path(), "9.9.9");
    let releases = release(dir.path(), "9.9.9", "herdr-linear", None);
    let cargo = fake_cargo(dir.path());
    let out = run(
        dir.path(),
        &root,
        &releases,
        Some(&cargo),
        &[("HERDR_LINEAR_BUILD_FROM_SOURCE", "1")],
    );
    assert!(out.status.success(), "{}", text(&out));
    assert!(!text(&out).contains("downloading"), "{}", text(&out));
    assert_eq!(installed(&root), "from-source");
}

#[test]
fn archive_without_the_binary_falls_back_to_cargo() {
    let dir = tempfile::tempdir().unwrap();
    let root = plugin_root(dir.path(), "9.9.9");
    let releases = release(dir.path(), "9.9.9", "something-else", None);
    let cargo = fake_cargo(dir.path());
    let out = run(dir.path(), &root, &releases, Some(&cargo), &[]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(
        text(&out).contains("does not contain herdr-linear"),
        "{}",
        text(&out)
    );
    assert_eq!(installed(&root), "from-source");
}

#[test]
fn runs_like_herdr_from_the_plugin_root_even_with_cdpath() {
    let dir = tempfile::tempdir().unwrap();
    let root = plugin_root(dir.path(), "9.9.9");
    let releases = release(dir.path(), "9.9.9", "herdr-linear", None);
    // herdr는 플러그인 루트에서 `bash scripts/install.sh`로 돌린다. CDPATH가 있어도 루트를 찾는다
    let out = Command::new("/bin/bash")
        .arg("scripts/install.sh")
        .current_dir(&root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir.path())
        .env("CDPATH", ".")
        .env(
            "HERDR_LINEAR_RELEASES",
            format!("file://{}", releases.display()),
        )
        .env("HERDR_LINEAR_TARGET", TARGET)
        .env("HERDR_LINEAR_RETRY_SECONDS", "0")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(installed(&root), "fake-release");
}

#[test]
fn failed_install_keeps_the_existing_binary() {
    let dir = tempfile::tempdir().unwrap();
    let root = plugin_root(dir.path(), "9.9.9");
    let bin = root.join("target/release/herdr-linear");
    fs::create_dir_all(bin.parent().unwrap()).unwrap();
    fs::write(&bin, "#!/bin/sh\necho old\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    let releases = release(
        dir.path(),
        "9.9.9",
        "herdr-linear",
        Some("0000000000000000000000000000000000000000000000000000000000000000"),
    );
    let out = run(dir.path(), &root, &releases, None, &[]);
    assert!(!out.status.success(), "{}", text(&out));
    assert_eq!(installed(&root), "old");
}
