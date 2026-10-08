# 미리 빌드한 바이너리 배포 (v0.2.1) 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vX.Y.Z` 태그를 push하면 macOS·Linux 바이너리와 SHA-256 체크섬이 GitHub Release에 올라가고, herdr가 설치할 때 그 바이너리를 받아 쓴다. Rust는 받지 못할 때만 필요하다. v0.2.1로 낸다.

**Architecture:**
- `scripts/install.sh`(새 파일)가 매니페스트 `[[build]]`에서 돌며 받기·확인·설치·소스 빌드 대체를 맡는다.
- `.github/workflows/release.yml`(새 파일)이 태그마다 빌드해 올린다. 태그와 버전 비교는 `scripts/check-release-version.sh`(새 파일)가 하고, 워크플로와 테스트가 같은 스크립트를 쓴다.
- Rust 코드(`src/`)는 바꾸지 않는다. 스크립트는 `tests/`의 통합 테스트가 `file://` 가짜 릴리스로 확인한다.

**Tech Stack:** bash(macOS 기본 3.2와 맞게), curl, tar, GitHub Actions(`actions/checkout`, `taiki-e/upload-rust-binary-action`), Rust 통합 테스트(tempfile). **새 Cargo 의존성은 없다.**

**Spec:** `docs/superpowers/specs/2026-10-09-prebuilt-binaries-design.md`

## 시작 전에

- 브랜치 `feat/prebuilt-binaries`(스펙 커밋 aac267e)에서 Task 1·2를 한다. Task 3·4는 C1 뒤에 새 브랜치에서 한다(아래 "진행 순서").
- 각 작업은 테스트 → 실패 확인 → 구현 → 통과 확인 → 커밋 순서다.
- 편집 블록 읽는 법: "찾아 바꾼다"의 첫 블록은 그 시점의 파일에 정확히 한 번 나온다(Edit 도구의 old_string/new_string).
- 작업마다 끝에서 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`가 통과해야 한다. 시작 시점에는 라이브러리 테스트 341개 통과, 1개 무시다.
- 커밋 메시지는 제목 줄과 꼬리말을 따로 넘긴다. 꼬리말은 자기 환경이 알려 주는 Co-Authored-By 줄을 쓴다.
- **구현 작업은 push·태그·워크플로 실행을 하지 않는다.** 바깥에 공개되는 일(C1~C3)은 컨트롤러가 한다.

## 진행 순서

1. Task 1 → Task 2 → 브랜치 전체 리뷰 → **C1**(main에 머지·push, 시험 빌드)
2. Task 3(새 브랜치 `release/v0.2.1`) → **C2**(`v0.2.1` 태그 먼저 push, 릴리스 확인) → **C3**(Rust 없는 설치와 네 바이너리 확인) → main에 머지·push
3. Task 4(ROADMAP) → main에 머지·push

## Global Constraints

- **의존성**: Cargo 의존성을 더하거나 바꾸지 않는다. `rust-version = "1.88"`.
- **bash 3.2**: `scripts/*.sh`는 macOS 기본 `/bin/bash`(3.2)에서 돈다. 연관 배열·`mapfile`·`${var,,}` 같은 bash 4 기능을 쓰지 않는다.
- **쓰는 명령**: `bash`, `curl`, `tar`, `awk`, `mktemp`, `install`, `mv`, `rm`, `mkdir`, `dirname`, `uname`, `sleep`, `sha256sum` 또는 `shasum`만 쓴다(`/usr/bin:/bin`에 있는 것).
- **이름**
  - 릴리스 파일: `herdr-linear-<target>.tar.gz`(압축 맨 위에 `herdr-linear`), `herdr-linear-<target>.sha256`(`<hex>  herdr-linear-<target>.tar.gz` 한 줄)
  - 받는 주소: `<RELEASES>/v<버전>/<파일>`, `<RELEASES>` 기본값 `https://github.com/jenthous/herdr-linear/releases/download`
  - target: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`
- **설치 위치**: `target/release/herdr-linear`(권한 0755). 매니페스트의 액션·pane 명령 경로는 바꾸지 않는다.
- **출력**: 설치 스크립트의 출력은 영어이고, 줄마다 `herdr-linear: `로 시작한다.
- **Action 고정**
  - `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`
  - `taiki-e/upload-rust-binary-action@f0d45ae91ee7b8ee928de7a9d04d893a08bcbec6 # v1.30.2`
- **테스트**: 네트워크를 쓰지 않는다(`file://` 가짜 릴리스). 셸 스크립트를 돌리는 테스트 파일은 맨 위에 `#![cfg(unix)]`를 둔다.

## Review Focus

1. **herdr의 빌드 작업 폴더**: herdr가 빌드 명령을 플러그인 루트가 아닌 폴더에서 돌려도, 바이너리는 플러그인 루트의 `target/release/`에 놓인다. (Task 1: 모든 테스트가 스크립트를 플러그인 루트 밖의 폴더에서 돌린다)
2. **잘못 올린 압축**: 압축 안에 `herdr-linear`가 없으면 설치하지 않고 소스 빌드로 넘어간다. (Task 1 `archive_without_the_binary_falls_back_to_cargo`)
3. **실패한 업데이트**: 받기나 확인이 실패해도 이미 있던 바이너리는 그대로 남는다. (Task 1 `failed_install_keeps_the_existing_binary`)
4. **비었거나 깨진 체크섬 파일**: 첫 칸이 해시가 아니면 불일치로 보고 설치하지 않는다. (Task 1 `bad_checksum_is_not_installed_and_explains_without_cargo`의 빈 값)
5. **macOS bash 3.2**: 스크립트가 macOS 기본 bash에서 돈다. (Task 1·2의 모든 테스트가 `/bin/bash`로 돌린다. 이 기기의 `/bin/bash`는 3.2다)

---

### Task 1: 설치 스크립트와 `[[build]]`

**Files:**
- Create: `scripts/install.sh`
- Create: `tests/install_script.rs`
- Modify: `herdr-plugin.toml` (`[[build]]`)

**Interfaces:**
- Produces: 실행 파일 `scripts/install.sh`. 인자 없음. 환경 변수(시험용) `HERDR_LINEAR_RELEASES`, `HERDR_LINEAR_TARGET`, `HERDR_LINEAR_RETRY_SECONDS`, `HERDR_LINEAR_BUILD_FROM_SOURCE`. 종료 코드 0 = 바이너리를 놓았거나 소스 빌드 성공, 1 = 둘 다 못 함.

- [ ] **Step 1: 테스트를 쓴다**

`tests/install_script.rs`를 만든다.

```rust
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
        .env("HERDR_LINEAR_RELEASES", format!("file://{}", releases.display()))
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
        assert!(!root.join("target/release/herdr-linear").exists(), "{bad:?}");
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
        .env("HERDR_LINEAR_RELEASES", format!("file://{}", releases.display()))
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
```

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --test install_script`
Expected: 7개 모두 실패한다. `scripts/install.sh`가 없어서 `fs::copy`가 패닉한다.

- [ ] **Step 3: 구현한다**

`scripts/install.sh`를 만든다.

```bash
#!/usr/bin/env bash
# herdr가 플러그인을 설치할 때 `[[build]]`에서 돌리는 설치 단계.
# 매니페스트 버전의 GitHub Release에서 이 기기용 바이너리를 받아 SHA-256을 확인하고
# target/release/herdr-linear에 놓는다. 쓸 수 있는 바이너리가 없으면 cargo로 빌드한다.
# macOS 기본 bash(3.2)와 /usr/bin·/bin의 명령만 쓴다.
set -euo pipefail

name="herdr-linear"
# herdr가 빌드 명령에 주는 작업 폴더·환경에 기대지 않고 스크립트 위치에서 플러그인 루트를 구한다
root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
dest="$root/target/release/$name"
releases="${HERDR_LINEAR_RELEASES:-https://github.com/jenthous/herdr-linear/releases/download}"
retry_seconds="${HERDR_LINEAR_RETRY_SECONDS:-3}"
version="$(awk -F'"' '/^version *=/ {print $2; exit}' "$root/herdr-plugin.toml")"

say() {
  printf '%s: %s\n' "$name" "$*"
}

# `uname -s`-`uname -m` → 릴리스 target. 미리 빌드한 바이너리가 없는 플랫폼이면 빈 값
target_for() {
  case "$1" in
    Darwin-arm64) echo "aarch64-apple-darwin" ;;
    Darwin-x86_64) echo "x86_64-apple-darwin" ;;
    Linux-x86_64) echo "x86_64-unknown-linux-musl" ;;
    Linux-aarch64 | Linux-arm64) echo "aarch64-unknown-linux-musl" ;;
    *) echo "" ;;
  esac
}

# 막 올린 릴리스는 몇 분 동안 404가 날 수 있어서 5번까지 다시 시도한다
fetch() {
  local attempt=1
  while ! curl -fsSL "$1" -o "$2"; do
    if [ "$attempt" -ge 5 ]; then
      return 1
    fi
    attempt=$((attempt + 1))
    sleep "$retry_seconds"
  done
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# 미리 빌드한 바이너리를 받아 놓는다. 하나라도 안 되면 이유를 알리고 1을 돌려준다.
# `if`의 조건으로 불려서 set -e가 듣지 않으므로 실패를 하나씩 직접 확인한다.
install_prebuilt() {
  local target archive sums expected actual
  if [ "${HERDR_LINEAR_BUILD_FROM_SOURCE:-}" = "1" ]; then
    say "HERDR_LINEAR_BUILD_FROM_SOURCE=1, skipping the prebuilt binary"
    return 1
  fi
  if [ -z "$version" ]; then
    say "could not read the version from herdr-plugin.toml"
    return 1
  fi
  target="${HERDR_LINEAR_TARGET:-$(target_for "$(uname -s)-$(uname -m)")}"
  if [ -z "$target" ]; then
    say "no prebuilt binary for $(uname -s) $(uname -m)"
    return 1
  fi
  archive="$name-$target.tar.gz"
  sums="$name-$target.sha256"
  say "downloading $archive for v$version"
  if ! fetch "$releases/v$version/$archive" "$tmp/$archive" ||
    ! fetch "$releases/v$version/$sums" "$tmp/$sums"; then
    say "could not download the prebuilt binary for v$version"
    return 1
  fi
  expected="$(awk 'NR == 1 {print $1}' "$tmp/$sums")"
  actual="$(sha256_of "$tmp/$archive")"
  if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    say "checksum mismatch for $archive, not installing it"
    return 1
  fi
  if ! mkdir -p "$tmp/unpacked" "$(dirname "$dest")" ||
    ! tar -xzf "$tmp/$archive" -C "$tmp/unpacked" ||
    [ ! -f "$tmp/unpacked/$name" ]; then
    say "$archive does not contain $name"
    return 1
  fi
  # 임시 이름에 쓰고 바꿔서, 도는 중인 바이너리가 있어도 반쯤 쓴 파일이 보이지 않게 한다
  if ! install -m 0755 "$tmp/unpacked/$name" "$dest.new" || ! mv -f "$dest.new" "$dest"; then
    rm -f "$dest.new"
    say "could not write $dest"
    return 1
  fi
  say "installed the prebuilt binary for v$version ($target)"
}

if install_prebuilt; then
  exit 0
fi
if command -v cargo >/dev/null 2>&1; then
  say "building from source with cargo, this takes a few minutes"
  cd "$root"
  cargo build --release
  exit 0
fi
say "cannot install: no usable prebuilt binary, and Rust (cargo) is not installed"
say "install Rust from https://rustup.rs and install the plugin again, or if v$version was released a moment ago, try again in a few minutes"
exit 1
```

Then make it executable: `chmod +x scripts/install.sh` (the tests run it through `/bin/bash`, but herdr may not).

`herdr-plugin.toml`에서 찾아 바꾼다.

```toml
[[build]]
command = ["cargo", "build", "--release"]
```

```toml
# 미리 빌드한 바이너리를 받는다. 쓸 수 없으면 cargo로 빌드한다
[[build]]
command = ["bash", "scripts/install.sh"]
```

- [ ] **Step 4: 통과를 확인한다**

Run: `cargo test --test install_script`
Expected: 7개 통과. 출력에 경고가 없다.

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: 모두 통과(라이브러리 341개 + 1개 무시, `install_script` 7개).

Run: `bash -n scripts/install.sh && /bin/bash --version | head -1`
Expected: 문법 오류 없음. 이 기기의 `/bin/bash`는 3.2다(테스트도 이 bash로 돌았다).

- [ ] **Step 5: 커밋한다**

```bash
git add scripts/install.sh tests/install_script.rs herdr-plugin.toml
git commit -m "feat: 설치 때 미리 빌드한 바이너리를 받고 체크섬 확인, 안 되면 cargo로 빌드" -m "<your Co-Authored-By trailer>"
```

---

### Task 2: 릴리스 워크플로와 버전 확인

**Files:**
- Create: `scripts/check-release-version.sh`
- Create: `tests/release_version.rs`
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: Task 1의 릴리스 파일 이름 규칙(Global Constraints)
- Produces
  - 실행 파일 `scripts/check-release-version.sh <태그>`: 태그(`vX.Y.Z`)에서 `v`를 뗀 값이 `Cargo.toml`과 `herdr-plugin.toml`의 첫 `version`과 같으면 0, 아니면 이유를 표준 에러에 쓰고 1.
  - 워크플로 `release`: 태그 push → 버전 확인 → 릴리스 생성 → 4개 플랫폼 업로드. `workflow_dispatch` → 4개 플랫폼 시험 빌드만.

- [ ] **Step 1: 테스트를 쓴다**

`tests/release_version.rs`를 만든다.

```rust
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
```

- [ ] **Step 2: 실패를 확인한다**

Run: `cargo test --test release_version`
Expected: 2개 실패(스크립트가 없어서 bash가 종료 코드 127을 낸다. 두 번째 테스트는 표준 에러에 "does not match"가 없어서 실패한다).

- [ ] **Step 3: 구현한다**

`scripts/check-release-version.sh`를 만들고 `chmod +x scripts/check-release-version.sh`.

```bash
#!/usr/bin/env bash
# 릴리스 태그(vX.Y.Z)가 Cargo.toml과 herdr-plugin.toml의 버전과 같은지 확인한다.
# 다르면 이유를 알리고 종료 코드 1. 릴리스 워크플로와 테스트가 함께 쓴다.
set -euo pipefail

tag="${1:?usage: check-release-version.sh vX.Y.Z}"
root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

first_version() {
  awk -F'"' '/^version *=/ {print $2; exit}' "$1"
}

want="${tag#v}"
cargo_version="$(first_version "$root/Cargo.toml")"
plugin_version="$(first_version "$root/herdr-plugin.toml")"
if [ "$cargo_version" != "$want" ] || [ "$plugin_version" != "$want" ]; then
  echo "release tag $tag does not match Cargo.toml ($cargo_version) and herdr-plugin.toml ($plugin_version)" >&2
  exit 1
fi
echo "release tag $tag matches Cargo.toml and herdr-plugin.toml"
```

`.github/workflows/release.yml`을 만든다.

```yaml
# 릴리스: vX.Y.Z 태그를 push하면 macOS·Linux 바이너리와 SHA-256 체크섬을 그 태그의 GitHub Release에 올린다.
# 시험 빌드: Actions 탭이나 `gh workflow run release.yml`로 돌리면 릴리스 없이 빌드·압축만 한다.
name: release

on:
  push:
    tags:
      - "v[0-9]+.[0-9]+.[0-9]+"
  workflow_dispatch:

permissions:
  contents: write

jobs:
  check-version:
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - run: bash scripts/check-release-version.sh "$GITHUB_REF_NAME"

  create-release:
    needs: check-version
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - run: gh release create "$GITHUB_REF_NAME" --title "$GITHUB_REF_NAME" --generate-notes --verify-tag
        env:
          GH_TOKEN: ${{ github.token }}

  upload-assets:
    needs: create-release
    # 시험 빌드에서는 앞 두 작업이 건너뛰어져도 돈다. 앞 작업이 실패하면 돌지 않는다
    if: ${{ !failure() && !cancelled() }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
          - target: aarch64-unknown-linux-musl
            os: ubuntu-latest
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: taiki-e/upload-rust-binary-action@f0d45ae91ee7b8ee928de7a9d04d893a08bcbec6 # v1.30.2
        with:
          bin: herdr-linear
          target: ${{ matrix.target }}
          checksum: sha256
          locked: true
          dry-run: ${{ github.event_name != 'push' }}
          token: ${{ github.token }}
```

- [ ] **Step 4: 통과를 확인한다**

Run: `cargo test --test release_version`
Expected: 2개 통과.

Run: `bash scripts/check-release-version.sh v0.2.0; echo "exit=$?"`
Expected: `release tag v0.2.0 matches Cargo.toml and herdr-plugin.toml`, `exit=0` (아직 0.2.0이다).

Run: `python3 -c 'import yaml; d = yaml.safe_load(open(".github/workflows/release.yml")); print(sorted(d["jobs"]), [m["target"] for m in d["jobs"]["upload-assets"]["strategy"]["matrix"]["include"]])'`
Expected: `['check-version', 'create-release', 'upload-assets'] ['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-musl', 'aarch64-unknown-linux-musl']`

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: 모두 통과.

- [ ] **Step 5: 커밋한다**

```bash
git add scripts/check-release-version.sh tests/release_version.rs .github/workflows/release.yml
git commit -m "ci: 태그를 push하면 macOS·Linux 바이너리와 체크섬을 릴리스에 올리기" -m "<your Co-Authored-By trailer>"
```

---

### C1: main에 머지·push하고 시험 빌드 (컨트롤러)

Task 1·2와 브랜치 전체 리뷰가 끝난 뒤 컨트롤러가 한다. 사용자가 "알아서 진행"으로 위임했다(2026-10-08).

```bash
git checkout main && git merge --ff-only feat/prebuilt-binaries
cargo test
git push origin main
gh workflow run release.yml --ref main
sleep 5; gh run list --workflow release.yml --limit 1
gh run watch <run-id> --exit-status
```

Expected: `upload-assets` 4개가 성공한다. `check-version`·`create-release`는 건너뛴다(태그 push가 아니다). 릴리스는 생기지 않는다.

실패하면 `gh run view <run-id> --log-failed`로 원인을 본다. 고친 커밋을 main에 push하고 다시 돌린다. musl 크로스 빌드가 실패하면 그 플랫폼의 matrix에 `build-tool: cargo-zigbuild`를 더하고, `upload-rust-binary-action` 단계에 `build-tool: ${{ matrix.build-tool }}`를 넘긴다(스펙 8장).

---

### Task 3: 버전 0.2.1과 README

C1이 성공한 뒤 `main`에서 새 브랜치 `release/v0.2.1`을 만들어 한다.

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock`, `herdr-plugin.toml` (버전)
- Modify: `README.md` (필요한 것·설치·개발, 영어·한국어)
- Modify: `docs/superpowers/specs/2026-10-06-herdr-linear-design.md` (3.1 `[[build]]`)

**Interfaces:**
- Consumes: Task 2의 `scripts/check-release-version.sh` (버전이 맞는지 테스트가 확인한다)

- [ ] **Step 1: 버전을 올린다**

`Cargo.toml`에서 `version = "0.2.0"` → `version = "0.2.1"`. `herdr-plugin.toml`에서 `version = "0.2.0"` → `version = "0.2.1"`.

Run: `cargo test`
Expected: 모두 통과. `Cargo.lock`의 herdr-linear 버전도 0.2.1로 바뀐다(`git diff --stat Cargo.lock`은 한 줄). `release_version`의 `cargo_and_plugin_versions_match_the_release_tag`가 두 파일이 같이 올라갔는지 확인한다.

- [ ] **Step 2: README 영어 부분을 고친다**

찾아 바꾼다.

```markdown
- herdr 0.9.3 or later
- Rust 1.88 or later (herdr builds the plugin from source when you install it)
- macOS or Linux
```

```markdown
- herdr 0.9.3 or later
- macOS (Apple Silicon or Intel) or Linux (x86_64 or arm64), with `bash` and `curl` (preinstalled on macOS and most Linux distributions)
- Rust 1.88 or later, only when no prebuilt binary can be used (another platform, or the download fails). The plugin is then built from source.
```

찾아 바꾼다(설치 명령 블록 바로 뒤에 한 문단을 넣는다).

````markdown
herdr plugin install jenthous/herdr-linear
```

Plugins can't register keys themselves.
````

````markdown
herdr plugin install jenthous/herdr-linear
```

On macOS and Linux this downloads a prebuilt binary from GitHub Releases and checks its SHA-256 checksum. If that isn't possible, it builds from source with Cargo.

Plugins can't register keys themselves.
````

찾아 바꾼다.

````markdown
scripts/deploy-local.sh   # build and link a stable copy for daily use on this machine
```
````

````markdown
scripts/deploy-local.sh   # build and link a stable copy for daily use on this machine
```

To release, bump `version` in `Cargo.toml` and `herdr-plugin.toml` (Cargo updates `Cargo.lock`), commit, and push the tag `vX.Y.Z` first. When the release workflow has attached the binaries and checksums, push `main`.
````

- [ ] **Step 3: README 한국어 부분을 고친다**

찾아 바꾼다.

```markdown
- herdr 0.9.3 이상
- Rust 1.88 이상 (설치할 때 herdr가 소스를 빌드해요)
- macOS 또는 Linux
```

```markdown
- herdr 0.9.3 이상
- macOS(Apple Silicon·Intel) 또는 Linux(x86_64·arm64), 그리고 `bash`·`curl`(macOS와 대부분의 Linux에 기본으로 있어요)
- Rust 1.88 이상은 미리 빌드한 바이너리를 쓸 수 없을 때만 필요해요(다른 플랫폼이거나 받기에 실패할 때). 그때는 소스로 빌드해요.
```

찾아 바꾼다.

````markdown
herdr plugin install jenthous/herdr-linear
```

플러그인은 키를 직접 등록할 수 없어서
````

````markdown
herdr plugin install jenthous/herdr-linear
```

macOS·Linux에서는 GitHub Releases에서 미리 빌드한 바이너리를 받아 SHA-256 체크섬을 확인해요. 그게 안 되면 Cargo로 소스를 빌드해요.

플러그인은 키를 직접 등록할 수 없어서
````

찾아 바꾼다.

````markdown
scripts/deploy-local.sh   # 이 기기에서 매일 쓸 복사본을 빌드해 herdr에 연결
```
````

````markdown
scripts/deploy-local.sh   # 이 기기에서 매일 쓸 복사본을 빌드해 herdr에 연결
```

릴리스는 `Cargo.toml`과 `herdr-plugin.toml`의 `version`을 올려(`Cargo.lock`은 Cargo가 맞춰요) 커밋하고, `vX.Y.Z` 태그를 먼저 push해요. 릴리스 워크플로가 바이너리와 체크섬을 올리면 그다음에 `main`을 push해요.
````

- [ ] **Step 4: 바탕 스펙 3.1을 고친다**

`docs/superpowers/specs/2026-10-06-herdr-linear-design.md`에서 찾아 바꾼다.

```toml
[[build]]
command = ["cargo", "build", "--release"]
```

```toml
# 미리 빌드한 바이너리를 받고, 안 되면 cargo로 빌드한다(2026-10-09-prebuilt-binaries-design.md)
[[build]]
command = ["bash", "scripts/install.sh"]
```

- [ ] **Step 5: 확인하고 커밋한다**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && bash scripts/check-release-version.sh v0.2.1`
Expected: 모두 통과하고 마지막 줄이 `release tag v0.2.1 matches Cargo.toml and herdr-plugin.toml`.

```bash
git add Cargo.toml Cargo.lock herdr-plugin.toml README.md docs/superpowers/specs/2026-10-06-herdr-linear-design.md
git commit -m "release: v0.2.1 — 미리 빌드한 바이너리 설치 안내" -m "<your Co-Authored-By trailer>"
```

---

### C2: v0.2.1 릴리스 (컨트롤러)

태그를 먼저 올리고, 자산과 C3 확인이 끝난 뒤에 main을 올린다. 그 사이 main에서 설치하는 사람이 아직 없는 v0.2.1 바이너리를 찾지 않게 하려는 것이다(최종 리뷰 반영).

```bash
git checkout release/v0.2.1
cargo build --release --locked   # 워크플로가 --locked로 빌드하므로 Cargo.lock이 맞는지 먼저 본다
gh api repos/jenthous/herdr-linear/immutable-releases --jq .enabled   # false여야 한다(릴리스를 먼저 공개하고 파일을 나중에 올린다)
git tag -a v0.2.1 -m "v0.2.1 — 미리 빌드한 바이너리(macOS·Linux)와 SHA-256 체크섬, 설치 때 받고 안 되면 소스 빌드"
git push origin v0.2.1
sleep 5; gh run list --workflow release.yml --limit 1
gh run watch <run-id> --exit-status
gh release view v0.2.1 --json assets --jq '.assets[].name' | sort
```

Expected: 워크플로의 모든 작업이 성공한다. 자산은 아래 8개다.

```
herdr-linear-aarch64-apple-darwin.sha256
herdr-linear-aarch64-apple-darwin.tar.gz
herdr-linear-aarch64-unknown-linux-musl.sha256
herdr-linear-aarch64-unknown-linux-musl.tar.gz
herdr-linear-x86_64-apple-darwin.sha256
herdr-linear-x86_64-apple-darwin.tar.gz
herdr-linear-x86_64-unknown-linux-musl.sha256
herdr-linear-x86_64-unknown-linux-musl.tar.gz
```

업로드가 실패하면
- `gh run rerun <run-id> --failed`로 실패한 작업만 다시 돌린다(업로드는 `--clobber`라 겹쳐 써도 된다).
- 전체를 다시 돌리면 `create-release`가 "already exists"로 막힌다. 그때는 `gh release delete v0.2.1`(태그는 지우지 않는다) 뒤에 다시 돌린다.
- 워크플로 파일을 고쳐야 하면 이미 올린 태그로는 고친 파일이 돌지 않는다. 태그를 옮기지 않고 v0.2.2로 낸다.

C3이 끝나면 main을 올린다.

```bash
git checkout main && git merge --ff-only release/v0.2.1 && git push origin main
```

---

### C3: Rust 없는 설치 확인 (컨트롤러)

이 기기의 herdr 플러그인은 건드리지 않는다. 설치 스크립트로 이 기기용 바이너리를 받아 보고, 나머지 세 바이너리도 이 기기에서 돌릴 수 있는 만큼 돌려 본다(최종 리뷰 반영: 체크섬이 맞으면 깨진 바이너리도 설치되기 때문).

```bash
T="$(mktemp -d)"
git clone -q --depth 1 --branch v0.2.1 https://github.com/jenthous/herdr-linear.git "$T/p"
env -i PATH=/usr/bin:/bin HOME="$T" /bin/bash "$T/p/scripts/install.sh"
"$T/p/target/release/herdr-linear" --version
gh release download v0.2.1 --repo jenthous/herdr-linear --pattern '*.tar.gz' --dir "$T/rel"
for t in x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
  mkdir -p "$T/$t" && tar -xzf "$T/rel/herdr-linear-$t.tar.gz" -C "$T/$t"
done
arch -x86_64 "$T/x86_64-apple-darwin/herdr-linear" --version
file "$T/aarch64-unknown-linux-musl/herdr-linear" "$T/x86_64-unknown-linux-musl/herdr-linear"
docker run --rm --network none -v "$T:/r:ro" alpine:3.19 /r/aarch64-unknown-linux-musl/herdr-linear --version
rm -rf "$T"
```

Expected
- `herdr-linear: downloading herdr-linear-aarch64-apple-darwin.tar.gz for v0.2.1`
- `herdr-linear: installed the prebuilt binary for v0.2.1 (aarch64-apple-darwin)`
- `herdr-linear 0.2.1` 세 번(이 기기 바이너리, Rosetta로 돈 x86_64 macOS, alpine 컨테이너의 aarch64 Linux)
- `file`이 두 Linux 바이너리를 모두 `statically linked`로 보여 준다. x86_64 Linux는 amd64 alpine 이미지가 이미 있을 때만 컨테이너로 돌리고, 없으면 `file` 결과로 갈음한다.

`cargo`가 없는 `PATH`라서, 받지 못했다면 "cannot install"로 끝난다.

---

### Task 4: ROADMAP

C3이 성공한 뒤 `main`에서 새 브랜치 `docs/roadmap-v0.2.1`을 만들어 한다.

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: 영어 부분을 고친다**

찾아 바꾼다.

```markdown
- The screen is redrawn only when something changes, and otherwise once a second, so an idle pane uses almost no CPU.
```

```markdown
- The screen is redrawn only when something changes, and otherwise once a second, so an idle pane uses almost no CPU.
- v0.2.1: prebuilt binaries for macOS and Linux with SHA-256 checksums on GitHub Releases, so installing doesn't need Rust. Building from source stays as the fallback.
```

아래 줄(`## Later` 아래)을 지운다.

```markdown
- Prebuilt binaries with checksums on GitHub Releases, so installing doesn't need Rust. Building from source stays as the fallback.
```

- [ ] **Step 2: 한국어 부분을 고친다**

찾아 바꾼다.

```markdown
- 바뀐 게 있을 때와 1초에 한 번만 다시 그려서, 가만히 둔 pane은 CPU를 거의 쓰지 않아요.
```

```markdown
- 바뀐 게 있을 때와 1초에 한 번만 다시 그려서, 가만히 둔 pane은 CPU를 거의 쓰지 않아요.
- v0.2.1: macOS·Linux용 미리 빌드한 바이너리와 SHA-256 체크섬을 GitHub Releases에 올려서 Rust 없이도 설치돼요. 소스 빌드는 대체 수단으로 남아요.
```

아래 줄(`## 나중에` 아래)을 지운다.

```markdown
- GitHub Releases에 미리 빌드한 바이너리와 체크섬을 올려서, Rust 없이도 설치되게 해요. 소스 빌드는 대체 수단으로 남겨요.
```

- [ ] **Step 3: 커밋한다**

Run: `grep -c "Prebuilt binaries with checksums\|미리 빌드한 바이너리와 체크섬을 올려서" ROADMAP.md`
Expected: `0`

```bash
git add ROADMAP.md
git commit -m "docs: ROADMAP — v0.2.1 미리 빌드한 바이너리 완료" -m "<your Co-Authored-By trailer>"
```

컨트롤러가 `main`에 fast-forward 머지하고 push한다.
