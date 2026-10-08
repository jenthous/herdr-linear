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

# 막 올린 릴리스는 몇 분 동안 404가 날 수 있어서 5번까지 시도한다.
# 연결이 멈추면 끝없이 기다리지 않게 연결 15초, 30초 동안 1KB/s 미만이면 그 시도를 포기한다.
# curl의 오류는 따로 받아 두었다가 마지막 이유만 우리 말머리로 알린다
fetch() {
  local attempt=1
  while ! curl -fsSL --connect-timeout 15 --speed-limit 1024 --speed-time 30 "$1" -o "$2" 2>"$tmp/curl.err"; do
    if [ "$attempt" -ge 5 ]; then
      say "could not download $1: $(awk 'END {print}' "$tmp/curl.err")"
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
  if ! command -v curl >/dev/null 2>&1; then
    say "curl is not installed, so the prebuilt binary cannot be downloaded"
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
