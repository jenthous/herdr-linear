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
