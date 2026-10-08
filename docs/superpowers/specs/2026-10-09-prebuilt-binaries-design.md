# 미리 빌드한 바이너리 배포 설계 (v0.2.1)

- 작성일: 2026-10-09
- 상태: 승인 (2026-10-08 대화로 설계 승인, 빌드는 GitHub Actions로 사용자 선택, 이후 "알아서 진행" 위임)
- 버전: v0.2.1
- 바탕 스펙: `docs/superpowers/specs/2026-10-06-herdr-linear-design.md` 3.1(매니페스트). ROADMAP "나중에"의 "GitHub Releases에 미리 빌드한 바이너리와 체크섬" 항목.

## 1. 목적

Rust가 없는 macOS·Linux 기기에서도 `herdr plugin install jenthous/herdr-linear` 한 번으로 설치된다. 소스 빌드는 대체 수단으로 남는다.

### 1.1 성공 기준

1. `vX.Y.Z` 태그를 push하면 GitHub Actions가 4개 플랫폼의 바이너리 압축 파일과 체크섬을 그 태그의 GitHub Release에 올린다.
2. 태그가 `Cargo.toml`·`herdr-plugin.toml`의 버전과 다르면 릴리스를 만들지 않는다.
3. herdr가 플러그인을 설치할 때 `[[build]]`가 설치 스크립트를 돌린다. 스크립트는 매니페스트 버전의 Release에서 이 기기용 파일을 받아, SHA-256이 맞을 때만 `target/release/herdr-linear`에 놓는다.
4. 받지 못했거나, 체크섬이 틀리거나, 미리 빌드한 바이너리가 없는 플랫폼이면 `cargo`가 있을 때 `cargo build --release`로 대신한다. `cargo`도 없으면 이유와 할 일을 알리고 실패한다.
5. 이 기기의 개발·배포 흐름(`scripts/deploy-local.sh`, `herdr plugin link`)은 그대로다. `herdr plugin link`는 `[[build]]`를 돌리지 않는다.
6. v0.2.1을 내고, Rust가 없는 PATH에서 설치 스크립트가 받은 바이너리가 실행되는 것을 확인한다.

### 1.2 범위 밖

- Windows (매니페스트 `platforms`가 macos·linux다)
- macOS 코드 서명·공증. `curl`로 받은 파일에는 격리 속성(quarantine)이 붙지 않아 Gatekeeper 확인을 거치지 않는다. Apple Silicon 바이너리는 링커가 ad-hoc 서명한다(이 기기의 release 빌드와 같다).
- 스스로 업데이트하기, `curl | sh` 설치기, Homebrew
- 손으로 쓰는 릴리스 노트 (GitHub 자동 생성 노트를 쓴다)

## 2. 플랫폼

| `uname -s`·`uname -m` | target | 빌드 |
|---|---|---|
| Darwin arm64 | `aarch64-apple-darwin` | `macos-latest`, cargo |
| Darwin x86_64 | `x86_64-apple-darwin` | `macos-latest`, cargo 크로스 빌드 |
| Linux x86_64 | `x86_64-unknown-linux-musl` | `ubuntu-latest`, cross |
| Linux aarch64·arm64 | `aarch64-unknown-linux-musl` | `ubuntu-latest`, cross |

- Linux는 musl 정적 빌드라서 배포판의 glibc 버전과 상관없이 돈다.
- rusqlite는 SQLite를 함께 빌드하고(`bundled`), TLS는 rustls(ring)다. 그래서 크로스 빌드에 대상 시스템의 라이브러리가 필요 없다.

## 3. 릴리스 워크플로 (`.github/workflows/release.yml`)

- **트리거**
  - `v[0-9]+.[0-9]+.[0-9]+` 태그 push: 릴리스를 만들고 파일을 올린다.
  - `workflow_dispatch`: 시험 빌드. 릴리스를 만들지 않고 빌드·압축만 한다.
- **권한**: `contents: write` (릴리스 만들기·파일 올리기)
- **Action은 커밋 해시로 고정한다**
  - `actions/checkout` v7.0.1: `3d3c42e5aac5ba805825da76410c181273ba90b1`
  - `taiki-e/upload-rust-binary-action` v1.30.2: `f0d45ae91ee7b8ee928de7a9d04d893a08bcbec6`
- **작업**
  1. `check-version` (태그 push 때만): 태그에서 `v`를 뗀 값이 `Cargo.toml`과 `herdr-plugin.toml`의 첫 `version` 값과 같지 않으면 실패한다.
  2. `create-release` (태그 push 때만): `gh release create <태그> --title <태그> --generate-notes --verify-tag`.
  3. `upload-assets` (2장의 4개 플랫폼): `upload-rust-binary-action`에 `bin: herdr-linear`, `target`, `checksum: sha256`, `locked: true`를 준다. 태그 push가 아니면 `dry-run: true`다. 앞 두 작업이 건너뛰어져도(시험 빌드) 돈다.
- **결과 파일** (플랫폼마다 두 개)
  - `herdr-linear-<target>.tar.gz`: 바이너리 `herdr-linear`가 압축의 맨 위에 있다.
  - `herdr-linear-<target>.sha256`: `sha256sum` 출력 한 줄(`<hex>  herdr-linear-<target>.tar.gz`).

## 4. 설치 스크립트 (`scripts/install.sh`)

매니페스트의 빌드 단계를 `[[build]] command = ["bash", "scripts/install.sh"]`로 바꾼다(지금은 `cargo build --release`).

### 4.1 흐름

1. **플러그인 루트**: 스크립트 위치에서 구한다. herdr가 빌드 명령에 어떤 작업 폴더·환경 변수를 주는지 기대지 않는다.
2. **버전**: `herdr-plugin.toml`의 첫 `version = "…"` 값.
3. **플랫폼**: `uname -s`·`uname -m`을 2장의 target으로 바꾼다. 표에 없으면 미리 빌드한 바이너리가 없는 것으로 본다.
4. **받기**
   - `<RELEASES>/v<버전>/herdr-linear-<target>.tar.gz`와 `<RELEASES>/v<버전>/herdr-linear-<target>.sha256`을 `curl -fsSL`로 받는다.
   - 막 올린 릴리스는 몇 분 동안 404가 날 수 있어서 파일마다 5번까지, 3초 간격으로 다시 시도한다.
   - `<RELEASES>` 기본값은 `https://github.com/jenthous/herdr-linear/releases/download`다.
5. **확인**: `.sha256` 첫 칸과 받은 압축 파일의 SHA-256이 같아야 한다. `sha256sum`이 없으면 `shasum -a 256`을 쓴다.
6. **놓기**: 임시 폴더에 압축을 풀고, `herdr-linear`를 `target/release/herdr-linear`에 권한 0755로 놓는다. 임시 이름으로 쓴 뒤 이름을 바꾼다. 액션·pane 명령의 경로(`./target/release/herdr-linear`)는 그대로다.
7. **대신 빌드**
   - 4~6이 안 되면, `cargo`가 있을 때 플러그인 루트에서 `cargo build --release`를 돌린다.
   - `cargo`도 없으면 아래를 알리고 종료 코드 1로 끝난다.
     - 미리 빌드한 바이너리를 쓸 수 없고 Rust(cargo)도 없다.
     - https://rustup.rs 에서 Rust를 설치하거나, 릴리스를 막 올렸다면 몇 분 뒤 다시 설치하라.
8. 임시 폴더는 끝날 때 지운다.

### 4.2 출력과 필요한 명령

- 출력은 영어이고, 줄마다 `herdr-linear: `로 시작한다. 설치 로그는 마켓플레이스 사용자 누구나 보므로 영어로 한다. 앱 화면 문구는 다국어 작업 때 바뀐다.
- 필요한 명령: `bash`, `curl`, `tar`, `awk`, `mktemp`, `install`, 그리고 `sha256sum`이나 `shasum`. macOS와 흔한 Linux에 기본으로 있다.

### 4.3 시험용 환경 변수

사용자 문서에는 적지 않는다. herdr가 설치 명령을 돌릴 때 사용자의 셸 환경을 넘기는지 알 수 없어서다.

| 변수 | 뜻 |
|---|---|
| `HERDR_LINEAR_RELEASES` | `<RELEASES>`를 바꾼다. `file://`도 된다 |
| `HERDR_LINEAR_TARGET` | target을 바꾼다 |
| `HERDR_LINEAR_RETRY_SECONDS` | 재시도 간격(초). 기본 3 |
| `HERDR_LINEAR_BUILD_FROM_SOURCE` | `1`이면 받지 않고 바로 소스 빌드로 간다 |

## 5. 테스트

- `tests/install_script.rs` (`#[cfg(unix)]`)
  - **준비**
    - 가짜 플러그인 루트: 임시 폴더에 `scripts/install.sh`를 복사하고 `herdr-plugin.toml`에 `version = "9.9.9"`를 쓴다.
    - 가짜 릴리스 폴더: `v9.9.9/herdr-linear-<target>.tar.gz`와 `.sha256`을 만든다. 압축 속 `herdr-linear`는 `fake-release`를 출력하는 셸 스크립트다.
    - `PATH`는 cargo가 없는 시스템 경로(`/usr/bin:/bin`)로 둔다.
  - **받은 바이너리를 놓는다**: `target/release/herdr-linear`가 0755로 생기고, 실행하면 `fake-release`를 출력한다.
  - **체크섬이 틀리면** 놓지 않는다. cargo가 없으면 종료 코드가 0이 아니고 Rust 안내가 나온다.
  - **소스 빌드로 넘어간다**: 릴리스에 그 버전의 파일이 없고 `PATH`에 가짜 `cargo`가 있으면, 플러그인 루트에서 `cargo build --release`가 불린다.
  - **`HERDR_LINEAR_BUILD_FROM_SOURCE=1`**이면 받지 않고 cargo를 부른다.
  - **버전 경로**: 매니페스트 버전이 받는 경로의 `v<버전>`이 된다. 다른 버전 폴더만 있으면 받지 못한다.
- `Cargo.toml`과 `herdr-plugin.toml`의 버전이 같은지 확인하는 테스트 (워크플로의 태그 확인과 같은 규칙)
- 워크플로는 `workflow_dispatch` 시험 빌드로 4개 플랫폼이 모두 빌드되는지 확인한다.

## 6. 릴리스 순서 (v0.2.1)

1. 구현을 main에 머지·push한다.
   - 이때 매니페스트 버전은 0.2.0이다. main에서 설치하면 v0.2.0 Release(바이너리 없음)를 찾다가 소스 빌드로 넘어간다. 지금과 결과가 같다.
2. main에서 `gh workflow run release.yml`로 시험 빌드를 돌린다. 4개 플랫폼이 모두 성공해야 한다. 실패하면 고치고 다시 돌린다.
3. 버전을 0.2.1로 올린다(`Cargo.toml`·`Cargo.lock`·`herdr-plugin.toml`). 같은 커밋에서 README를 고친다(7장). 그 커밋과 `v0.2.1` 태그를 함께 push한다.
4. 워크플로가 끝나면 Release에 파일 8개(압축 4개, 체크섬 4개)가 있는지 본다.
5. **확인**
   - 임시 폴더에 `v0.2.1`을 받아, cargo가 없는 `PATH`로 `bash scripts/install.sh`를 돌린다.
   - 받은 바이너리가 `herdr-linear 0.2.1`을 출력해야 한다.
   - 이 기기의 herdr에 깔린 플러그인은 건드리지 않는다.
6. ROADMAP을 고친다(7장). 다른 기기 업데이트 프롬프트를 사용자에게 남긴다.

## 7. 문서

- **README (영어·한국어)**
  - 필요한 것: "Rust 1.88 이상"을 "미리 빌드한 바이너리가 없을 때만 Rust 1.88 이상"으로 바꾼다. 다른 플랫폼이나 받기에 실패한 경우다.
  - 설치: macOS·Linux는 설치할 때 미리 빌드한 바이너리를 받고 체크섬을 확인한다고 적는다.
  - 개발: 릴리스 방법을 한 줄로 적는다. 두 파일의 버전을 올려 main에 push하고 `vX.Y.Z` 태그를 push하면 워크플로가 바이너리를 올린다.
- **ROADMAP (영어·한국어)**: "나중에"의 바이너리 항목을 지우고, v0.2 절에 "v0.2.1: 미리 빌드한 바이너리(macOS·Linux)와 체크섬" 줄을 더한다.
- **바탕 스펙 3.1**: 매니페스트 예시의 `[[build]]`를 설치 스크립트로 바꾸고, 이 문서를 가리킨다.

## 8. 위험과 대응

| 위험 | 대응 |
|---|---|
| herdr가 빌드 명령을 플러그인 루트가 아닌 곳에서 돌린다 | 스크립트가 자기 위치로 루트를 구한다 |
| 막 올린 Release가 잠깐 404를 낸다 | 다시 시도한다. 그래도 안 되면 소스 빌드나 안내로 넘어간다 |
| 받은 파일이 깨졌거나 바뀌었다 | SHA-256을 확인한다. 체크섬도 같은 Release에서 받으므로, 저장소 계정 자체가 털린 경우는 막지 못한다(범위 밖) |
| musl·SQLite 크로스 빌드가 실패한다 | 시험 빌드로 먼저 확인한다. 실패하면 그 플랫폼의 `build-tool`을 `cargo-zigbuild`로 바꾼다 |
| 릴리스 사이에 main에서 설치한다 | 매니페스트 버전의 Release를 찾는다. 아직 없으면 소스 빌드나 안내로 넘어간다 |
