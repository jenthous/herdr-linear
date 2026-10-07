#!/bin/sh
# herdr-linear을 이 기기에 배포한다.
# release 빌드 → 배포 위치에 매니페스트와 바이너리 복사 → herdr에 연결 → CLI 링크.
# 배포 위치를 따로 두어서, 저장소에서 개발·빌드해도 쓰고 있는 플러그인은 다시 배포할 때까지 그대로다.
# 플러그인 id(herdr-linear)는 같으므로 단축키, 키(credentials), 캐시는 그대로 쓴다.
set -eu

cd "$(dirname "$0")/.."
dest="${HERDR_LINEAR_DEPLOY_DIR:-$HOME/.local/share/herdr-linear/plugin}"
bin_dir="${HERDR_LINEAR_BIN_DIR:-$HOME/.local/bin}"
herdr="${HERDR_BIN_PATH:-herdr}"

cargo build --release
mkdir -p "$dest/target/release" "$bin_dir"
cp herdr-plugin.toml "$dest/herdr-plugin.toml"
# 실행 중인 팔레트가 있어도 안전하게 바꾸도록 새 파일을 만든 뒤 이름을 바꾼다
cp target/release/herdr-linear "$dest/target/release/herdr-linear.new"
mv -f "$dest/target/release/herdr-linear.new" "$dest/target/release/herdr-linear"

# 다른 위치(예: 개발 저장소)에 연결돼 있으면 배포 위치로 옮긴다
if ! "$herdr" plugin list 2>/dev/null | grep -F "herdr-linear (" | grep -qF "local:$dest]"; then
    "$herdr" plugin unlink herdr-linear >/dev/null 2>&1 || true
    "$herdr" plugin link "$dest" >/dev/null
fi
ln -sf "$dest/target/release/herdr-linear" "$bin_dir/herdr-linear"

echo "배포했어요: $dest"
"$herdr" plugin list 2>/dev/null | grep -F "herdr-linear (" || true
