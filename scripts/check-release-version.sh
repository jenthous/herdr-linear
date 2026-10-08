#!/usr/bin/env bash
# 릴리스 태그(vX.Y.Z)가 Cargo.toml·herdr-plugin.toml·Cargo.lock의 버전과 같은지 확인한다.
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
# 릴리스 빌드는 --locked라서 Cargo.lock의 herdr-linear 버전도 같아야 한다
lock_version="$(awk -F'"' '/^name = "herdr-linear"$/ {getline; print $2; exit}' "$root/Cargo.lock")"
if [ "$cargo_version" != "$want" ] || [ "$plugin_version" != "$want" ] || [ "$lock_version" != "$want" ]; then
  echo "release tag $tag does not match Cargo.toml ($cargo_version), herdr-plugin.toml ($plugin_version) and Cargo.lock ($lock_version)" >&2
  exit 1
fi
echo "release tag $tag matches Cargo.toml, herdr-plugin.toml and Cargo.lock"
