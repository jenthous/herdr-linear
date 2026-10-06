# herdr-linear 1부: 코어 + 조회 CLI 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** herdr-linear의 코어(설정·API 키, Linear API 클라이언트, 캐시, 검색, markdown 렌더러)를 만든다. 터미널 조회 CLI(`login`, `logout`, `whoami`, `mine`, `search`, `show`)로 실제 Linear 데이터에서 검증한다.

**Architecture:** 라이브러리 크레이트(`src/lib.rs`)와 얇은 바이너리(`src/main.rs`)로 나눈다. 모듈은 `config`, `linear`(client·types·queries·filter), `search`(query·rank), `store`, `markdown`, `cli`다. 2부의 TUI 팔레트가 같은 라이브러리를 그대로 쓴다. 네트워크는 블로킹 HTTP(ureq 3), 캐시는 SQLite(rusqlite bundled, WAL)다.

**Tech Stack:** Rust 2024 edition(rustc 1.94), ratatui 0.30.2, nucleo-matcher 0.3.1, rusqlite 0.40.2(bundled), pulldown-cmark 0.13.4, ureq 3.4.2, serde / serde_json, toml 1.1.6, clap 4.6.7, unicode-width 0.2.2, unicode-normalization 0.1.25, regex 1.13.1, chrono 0.4.45, anyhow, thiserror 2, rpassword 7.5.4. 테스트에는 mockito 1.7.2와 tempfile 3.27.0을 쓴다.

**Spec:** `docs/superpowers/specs/2026-10-06-herdr-linear-design.md`
- 이 계획은 스펙 16장 구현 순서의 2~3단계(기반, 읽기 경로)를 다룬다. 마지막 작업에서는 15장 가정 중 Linear 쪽 항목을 실제 키로 확인한다.
- 2부(팔레트 TUI + herdr 연결)와 3부(사이드 패널, 변경 동작, 에이전트 전달)는 이 계획이 끝난 뒤 별도 계획으로 쓴다.
- 스펙과 다른 점: 스펙의 CLI 진입점(`open`, `ui`, `logout`) 외에 개발·진단용 조회 명령(`login`, `whoami`, `mine`, `search`, `show`)을 추가한다. 2부 이후에도 유지한다.

> 이 계획의 코드는 작성 시점에 임시 크레이트에서 모두 확인했다.
> - `cargo test`: 117개 통과, 성능 테스트 1개는 별도 실행
> - `cargo clippy --all-targets -- -D warnings`: 경고 0개
> - `cargo fmt --check`: 통과

## 시작 전에

- 작업 브랜치를 만든다: `git switch -c feat/part1-core-cli` (또는 superpowers:using-git-worktrees로 worktree를 만든다).
- 도구: `cargo --version` 1.94 이상. 첫 빌드는 SQLite 소스를 컴파일하므로 C 컴파일러가 필요하다. macOS는 Xcode Command Line Tools를 쓴다.
- 테스트 작성 방식: 각 작업은 테스트 모듈부터 만든다(Step 1). 구현이 없으므로 컴파일이 실패하는 것을 확인하고(Step 2), 같은 파일 맨 위에 구현을 넣는다(Step 3). Rust에서는 "테스트 실패"가 대개 컴파일 실패로 나타난다.

## Global Constraints

- Rust edition 2024. 크레이트 버전은 Task 1의 `Cargo.toml` 그대로 쓰고, 메이저 버전은 올리지 않는다(ureq 3, rusqlite 0.40, ratatui 0.30, toml 1).
- 플랫폼: macOS, Linux.
- Linear 엔드포인트는 `https://api.linear.app/graphql`이고, 헤더는 `Authorization: <API 키>`(Bearer 없이)다.
- 키를 찾는 순서: `LINEAR_API_KEY` 환경 변수 → `credentials` 파일. credentials 권한은 0600, 설정 디렉터리 권한은 0700이다. 키는 로그, 화면, 에러 메시지에 남기지 않는다.
- 설정 경로 순서: `HERDR_PLUGIN_CONFIG_DIR` → `HERDR_LINEAR_CONFIG_DIR` → `~/.config/herdr-linear`.
- 상태 경로 순서: `HERDR_PLUGIN_STATE_DIR` → `HERDR_LINEAR_STATE_DIR` → `~/.local/state/herdr-linear`.
- 캐시: 상태 디렉터리의 `cache.db`, 권한 0600, WAL, `busy_timeout` 5초, `PRAGMA user_version = 1`. 버전이 다르면 비우고 새로 만든다.
- 타임아웃: 연결 5초, 전체 15초.
- 자동 요청은 남은 요청이 50 미만이면 멈춘다.
- 페이지 크기 50, 이슈당 라벨 20개, 코멘트 50개, 깊은 검색 20건.
- 캐시 보존 기간: `cache.retention_days`, 기본 30일.
- 문자열 비교는 NFC 정규화 후 대소문자를 무시한다. 표시 폭은 unicode-width 기준(한글 2칸)이다.
- 사용자에게 보이는 문구는 한국어로 쓴다.
- 모든 작업은 끝날 때 `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`를 통과해야 한다.

## Review Focus

스펙이 암시하지만 기능 테스트만으로는 놓치기 쉬운 입력·실패 상황이다. 각 항목의 테스트는 해당 작업에 들어 있다.

- **Linear 내용 속 터미널 제어 문자**: 제목·본문·이름에 ESC나 BEL이 들어 있으면, 그대로 출력할 때 화면이 지워지거나 클립보드 쓰기(OSC 52)가 실행된다. 출력 전에 지운다. (Task 9 `control_characters_are_removed`, Task 10 `list_and_detail_strip_control_characters`)
- **macOS에서 붙여넣은 NFD 한글**: 자모 단위로 쪼개져 줄바꿈과 비교가 어긋난다. 렌더링 전에 NFC로 정규화한다. (Task 9 `decomposed_hangul_is_normalized_before_wrapping`)
- **API 한도 초과**: "한도를 넘었어요"만 보이면 언제 다시 해야 할지 모른다. 리셋까지 남은 분을 알려준다. (Task 10 `rate_limit_says_when_to_retry`)
- **두 프로세스가 동시에 캐시에 쓸 때**: 2부에서는 팔레트와 사이드 패널이 함께 뜬다. 잠금 오류로 실패하면 안 되고, 열기 실패를 손상으로 오해해 캐시를 지워도 안 된다. 기다렸다가 쓰고, 파일이 실제로 손상됐을 때(NOTADB·CORRUPT)만 다시 만든다. (Task 7 `two_writers_wait_instead_of_failing`, `only_corruption_triggers_recreate`)
- **아주 좁은 터미널, 열이 많은 표**: 폭 계산이 어긋나 패닉이 나면 안 된다. 폭은 최소 10칸으로 보고, 표 열은 3칸 밑으로 줄이지 않는다. (Task 9 `tiny_width_and_wide_tables_do_not_panic`)

## 파일 구조

| 파일 | 책임 |
|---|---|
| `Cargo.toml` | 의존성(버전 고정), release 프로필 |
| `src/lib.rs` | 모듈 선언 |
| `src/main.rs` | CLI 진입점(인자 해석 → `cli::run` → 출력·종료 코드) |
| `src/config.rs` | 경로 결정, `config.toml` 읽기, API 키 읽기·저장·삭제, 비공개 파일 쓰기 |
| `src/linear/types.rs` | Linear 응답 타입(Float → i64 변환 포함) |
| `src/linear/client.rs` | HTTP 전송, 오류 분류, 한도 헤더 추적 |
| `src/linear/queries.rs` | GraphQL 문서와 호출 함수 |
| `src/linear/filter.rs` | 검색어 → `IssueFilter` JSON |
| `src/search/query.rs` | 검색어 해석(자유 텍스트 + `s:` `l:` `@` `#` `p:` 토큰) |
| `src/search/rank.rs` | 로컬 매칭(nucleo), 정렬, 서버 결과 합치기 |
| `src/store/mod.rs` | SQLite 열기, 스키마, 손상 복구 |
| `src/store/cache.rs` | 이슈·코멘트·보기 결과·브랜치 매핑 읽기/쓰기 |
| `src/markdown.rs` | markdown → ratatui `Line`, 제어 문자 제거, 평문·ANSI 출력 |
| `src/cli.rs` | 조회 명령과 출력 형식 |
| `src/test_support.rs` | 테스트용 이슈 빌더(`#[cfg(test)]`) |

---

### Task 1: 크레이트 뼈대 + 경로·설정·API 키

**Files:**
- Create: `Cargo.toml`, `src/lib.rs`, `src/config.rs`
- Modify: `.gitignore`(`/target/` 추가)
- Test: `src/config.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 없음
- Produces:
  - `config::PLUGIN_ID: &str = "herdr-linear"`, `config::DEFAULT_AGENT_TEMPLATE: &str`
  - `config::Paths { config_dir: PathBuf, state_dir: PathBuf }`
  - `Paths::from_env() -> Result<Paths>`, `Paths::resolve(get: impl Fn(&str) -> Option<String>) -> Result<Paths>`
  - `Paths::config_file()`, `credentials_file()`, `cache_db()`, `log_file()` (모두 `-> PathBuf`)
  - `config::Settings { teams: Vec<String>, side_refresh_seconds: u64, cache_retention_days: u64, agent_include_comments: u64, agent_template: String }` (`Default` 구현: 60, 30, 0, 기본 템플릿)
  - `config::load_settings(&Path) -> (Settings, Vec<String>)` (두 번째 값은 경고 문구)
  - `config::KeySource::{Env, File}`, `config::ApiKey { value: String, source: KeySource }` (`Debug`에 값이 드러나지 않음)
  - `config::resolve_api_key(env_value: Option<String>, &Paths) -> Result<Option<ApiKey>>`
  - `config::save_api_key(&Paths, &str) -> Result<()>`, `config::delete_credentials(&Paths) -> Result<()>`
  - `config::ensure_private_dir(&Path) -> Result<()>`(0700), `config::write_private_file(&Path, &[u8]) -> Result<()>`(0600)

- [ ] **Step 1: 크레이트 파일과 실패하는 테스트 작성**

`Cargo.toml`:

````toml
[package]
name = "herdr-linear"
version = "0.1.0"
edition = "2024"
description = "Fast Linear lookup for herdr"
license = "MIT"

[dependencies]
anyhow = "1.0.104"
chrono = { version = "0.4.45", default-features = false, features = ["std", "clock"] }
clap = { version = "4.6.7", features = ["derive"] }
nucleo-matcher = "0.3.1"
pulldown-cmark = { version = "0.13.4", default-features = false }
ratatui = "0.30.2"
regex = "1.13.1"
rpassword = "7.5.4"
rusqlite = { version = "0.40.2", features = ["bundled"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.21"
toml = "1.1.6"
unicode-normalization = "0.1.25"
unicode-width = "0.2.2"
ureq = { version = "3.4.2", features = ["json"] }

[dev-dependencies]
mockito = "1.7.2"
tempfile = "3.27.0"

[profile.release]
lto = "thin"
strip = true
````

`.gitignore` 끝에 한 줄 추가:

````text
/target/
````

`src/lib.rs`:

````rust
//! herdr-linear: herdr 안에서 Linear를 빠르게 조회한다.

pub mod config;
````

`src/config.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    fn temp_paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            state_dir: dir.path().join("state"),
        };
        (dir, paths)
    }

    #[test]
    fn herdr_plugin_dirs_win() {
        let p = Paths::resolve(env(&[
            ("HERDR_PLUGIN_CONFIG_DIR", "/h/config"),
            ("HERDR_PLUGIN_STATE_DIR", "/h/state"),
            ("HERDR_LINEAR_CONFIG_DIR", "/l/config"),
            ("HOME", "/home/me"),
        ]))
        .unwrap();
        assert_eq!(p.config_dir, PathBuf::from("/h/config"));
        assert_eq!(p.state_dir, PathBuf::from("/h/state"));
    }

    #[test]
    fn linear_dirs_used_outside_herdr() {
        let p = Paths::resolve(env(&[
            ("HERDR_LINEAR_CONFIG_DIR", "/l/config"),
            ("HERDR_LINEAR_STATE_DIR", "/l/state"),
            ("HOME", "/home/me"),
        ]))
        .unwrap();
        assert_eq!(p.config_dir, PathBuf::from("/l/config"));
        assert_eq!(p.state_dir, PathBuf::from("/l/state"));
    }

    #[test]
    fn home_fallback_and_empty_values_ignored() {
        let p = Paths::resolve(env(&[
            ("HERDR_PLUGIN_CONFIG_DIR", "  "),
            ("HOME", "/home/me"),
        ]))
        .unwrap();
        assert_eq!(p.config_dir, PathBuf::from("/home/me/.config/herdr-linear"));
        assert_eq!(
            p.state_dir,
            PathBuf::from("/home/me/.local/state/herdr-linear")
        );
        assert_eq!(
            p.cache_db(),
            PathBuf::from("/home/me/.local/state/herdr-linear/cache.db")
        );
        assert_eq!(
            p.credentials_file(),
            PathBuf::from("/home/me/.config/herdr-linear/credentials")
        );
    }

    #[test]
    fn no_home_is_an_error() {
        assert!(Paths::resolve(env(&[])).is_err());
    }

    #[test]
    fn settings_default_when_file_missing() {
        let (_d, paths) = temp_paths();
        let (s, w) = load_settings(&paths.config_file());
        assert_eq!(s, Settings::default());
        assert!(w.is_empty());
        assert_eq!(s.side_refresh_seconds, 60);
        assert_eq!(s.cache_retention_days, 30);
        assert!(s.agent_template.contains("{identifier}"));
    }

    #[test]
    fn settings_read_all_values() {
        let (_d, paths) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            paths.config_file(),
            "teams = [\"eng\", \" ops \"]\n[side]\nrefresh_seconds = 0\n[cache]\nretention_days = 7\n[agent]\ninclude_comments = 3\ntemplate = \"{title}\"\n",
        )
        .unwrap();
        let (s, w) = load_settings(&paths.config_file());
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(s.teams, vec!["ENG", "OPS"]);
        assert_eq!(s.side_refresh_seconds, 0);
        assert_eq!(s.cache_retention_days, 7);
        assert_eq!(s.agent_include_comments, 3);
        assert_eq!(s.agent_template, "{title}");
    }

    #[test]
    fn invalid_item_falls_back_with_warning() {
        let (_d, paths) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            paths.config_file(),
            "teams = \"ENG\"\n[side]\nrefresh_seconds = \"fast\"\n[cache]\nretention_days = 0\n",
        )
        .unwrap();
        let (s, w) = load_settings(&paths.config_file());
        assert_eq!(s, Settings::default());
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(w.iter().any(|m| m.contains("teams")));
        assert!(w.iter().any(|m| m.contains("side.refresh_seconds")));
        assert!(w.iter().any(|m| m.contains("cache.retention_days")));
    }

    #[test]
    fn broken_toml_uses_defaults() {
        let (_d, paths) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.config_file(), "teams = [").unwrap();
        let (s, w) = load_settings(&paths.config_file());
        assert_eq!(s, Settings::default());
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn env_key_wins_over_file() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "lin_api_file").unwrap();
        let k = resolve_api_key(Some(" lin_api_env ".into()), &paths)
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_env");
        assert_eq!(k.source, KeySource::Env);
    }

    #[test]
    fn file_key_used_when_env_empty() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "  lin_api_file  ").unwrap();
        let k = resolve_api_key(Some("".into()), &paths).unwrap().unwrap();
        assert_eq!(k.value, "lin_api_file");
        assert_eq!(k.source, KeySource::File);
    }

    #[test]
    fn missing_or_blank_file_means_no_key() {
        let (_d, paths) = temp_paths();
        assert_eq!(resolve_api_key(None, &paths).unwrap(), None);
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.credentials_file(), "  \n").unwrap();
        assert_eq!(resolve_api_key(None, &paths).unwrap(), None);
    }

    #[test]
    fn empty_key_is_rejected() {
        let (_d, paths) = temp_paths();
        assert!(save_api_key(&paths, "   ").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn credentials_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "lin_api_x").unwrap();
        let file_mode = fs::metadata(paths.credentials_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let dir_mode = fs::metadata(&paths.config_dir)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
    }

    #[test]
    fn delete_is_idempotent() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "lin_api_x").unwrap();
        delete_credentials(&paths).unwrap();
        delete_credentials(&paths).unwrap();
        assert_eq!(resolve_api_key(None, &paths).unwrap(), None);
    }

    #[test]
    fn debug_never_shows_key() {
        let k = ApiKey {
            value: "lin_api_secret".into(),
            source: KeySource::File,
        };
        let shown = format!("{k:?}");
        assert!(!shown.contains("secret"));
        assert!(shown.contains("<redacted>"));
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test config::`
Expected: 컴파일 실패 — `cannot find type `Paths` in this scope` 같은 오류(구현이 아직 없음)

- [ ] **Step 3: 구현 작성**

`src/config.rs` 맨 위(`#[cfg(test)]` 위)에 넣는다:

````rust
//! 설정 파일, 경로, API 키.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

pub const PLUGIN_ID: &str = "herdr-linear";

pub const DEFAULT_AGENT_TEMPLATE: &str = "{instruction}\n\nLinear 이슈 {identifier}: {title}\n{url}\n상태: {state} · 우선순위: {priority} · 라벨: {labels}\n\n{description}\n{comments}\n";

/// 설정·상태 디렉터리.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
}

impl Paths {
    /// 실제 환경 변수로 경로를 정한다.
    pub fn from_env() -> Result<Paths> {
        Paths::resolve(|name| std::env::var(name).ok())
    }

    /// `get`으로 환경 변수를 읽어 경로를 정한다. 비어 있는 값은 없는 것으로 본다.
    /// 순서: herdr가 준 플러그인 디렉터리 → HERDR_LINEAR_* → HOME 아래 기본 위치.
    pub fn resolve(get: impl Fn(&str) -> Option<String>) -> Result<Paths> {
        let var = |name: &str| {
            get(name)
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from)
        };
        let home = var("HOME");
        let config_dir = var("HERDR_PLUGIN_CONFIG_DIR")
            .or_else(|| var("HERDR_LINEAR_CONFIG_DIR"))
            .or_else(|| home.as_ref().map(|h| h.join(".config").join(PLUGIN_ID)))
            .ok_or_else(|| anyhow!("설정 디렉터리를 정할 수 없어요 (HOME이 없어요)"))?;
        let state_dir = var("HERDR_PLUGIN_STATE_DIR")
            .or_else(|| var("HERDR_LINEAR_STATE_DIR"))
            .or_else(|| {
                home.as_ref()
                    .map(|h| h.join(".local").join("state").join(PLUGIN_ID))
            })
            .ok_or_else(|| anyhow!("상태 디렉터리를 정할 수 없어요 (HOME이 없어요)"))?;
        Ok(Paths {
            config_dir,
            state_dir,
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn credentials_file(&self) -> PathBuf {
        self.config_dir.join("credentials")
    }

    pub fn cache_db(&self) -> PathBuf {
        self.state_dir.join("cache.db")
    }

    pub fn log_file(&self) -> PathBuf {
        self.state_dir.join("herdr-linear.log")
    }
}

/// config.toml 값. 모든 항목은 선택이고 기본값이 있다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// 범위 팀 키. 비어 있으면 내가 속한 팀 전부.
    pub teams: Vec<String>,
    pub side_refresh_seconds: u64,
    pub cache_retention_days: u64,
    pub agent_include_comments: u64,
    pub agent_template: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            teams: Vec::new(),
            side_refresh_seconds: 60,
            cache_retention_days: 30,
            agent_include_comments: 0,
            agent_template: DEFAULT_AGENT_TEMPLATE.to_string(),
        }
    }
}

/// config.toml을 읽는다. 파일이 없으면 기본값을 쓴다.
/// 잘못된 항목은 그 항목만 기본값을 쓰고, 경고 문구를 함께 돌려준다.
pub fn load_settings(path: &Path) -> (Settings, Vec<String>) {
    let mut s = Settings::default();
    let mut warnings = Vec::new();
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (s, warnings),
        Err(e) => {
            warnings.push(format!("config.toml을 읽지 못했어요: {e}"));
            return (s, warnings);
        }
    };
    let table: toml::Table = match toml::from_str(&text) {
        Ok(t) => t,
        Err(e) => {
            warnings.push(format!("config.toml 형식이 잘못돼서 기본값을 써요: {e}"));
            return (s, warnings);
        }
    };
    if let Some(v) = table.get("teams") {
        let keys = v.as_array().and_then(|items| {
            items
                .iter()
                .map(|x| x.as_str().map(|k| k.trim().to_uppercase()))
                .collect::<Option<Vec<_>>>()
        });
        match keys {
            Some(keys) => s.teams = keys.into_iter().filter(|k| !k.is_empty()).collect(),
            None => warnings.push("teams는 문자열 배열이어야 해요. 기본값을 써요".to_string()),
        }
    }
    read_u64(
        &table,
        "side",
        "refresh_seconds",
        0,
        &mut s.side_refresh_seconds,
        &mut warnings,
    );
    read_u64(
        &table,
        "cache",
        "retention_days",
        1,
        &mut s.cache_retention_days,
        &mut warnings,
    );
    read_u64(
        &table,
        "agent",
        "include_comments",
        0,
        &mut s.agent_include_comments,
        &mut warnings,
    );
    if let Some(v) = table.get("agent").and_then(|a| a.get("template")) {
        match v.as_str() {
            Some(t) => s.agent_template = t.to_string(),
            None => warnings.push("agent.template은 문자열이어야 해요. 기본값을 써요".to_string()),
        }
    }
    (s, warnings)
}

fn read_u64(
    table: &toml::Table,
    section: &str,
    key: &str,
    min: u64,
    target: &mut u64,
    warnings: &mut Vec<String>,
) {
    let Some(v) = table.get(section).and_then(|t| t.get(key)) else {
        return;
    };
    match v.as_integer() {
        Some(n) if n >= min as i64 => *target = n as u64,
        _ => warnings.push(format!(
            "{section}.{key}는 {min} 이상의 정수여야 해요. 기본값을 써요"
        )),
    }
}

/// 키를 어디서 얻었는지.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Env,
    File,
}

/// Linear Personal API 키. Debug 출력에 값이 드러나지 않는다.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey {
    pub value: String,
    pub source: KeySource,
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKey")
            .field("value", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

/// 키를 찾는다. 순서: `LINEAR_API_KEY` 환경 변수 → credentials 파일.
pub fn resolve_api_key(env_value: Option<String>, paths: &Paths) -> Result<Option<ApiKey>> {
    if let Some(v) = env_value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Ok(Some(ApiKey {
            value: v,
            source: KeySource::Env,
        }));
    }
    match fs::read_to_string(paths.credentials_file()) {
        Ok(text) => {
            let v = text.trim().to_string();
            Ok((!v.is_empty()).then_some(ApiKey {
                value: v,
                source: KeySource::File,
            }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("credentials 파일을 읽지 못했어요"),
    }
}

/// 키를 credentials 파일에 0600 권한으로 저장한다.
pub fn save_api_key(paths: &Paths, key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(anyhow!("빈 키는 저장할 수 없어요"));
    }
    ensure_private_dir(&paths.config_dir)?;
    write_private_file(&paths.credentials_file(), format!("{key}\n").as_bytes())
}

/// credentials 파일을 지운다. 없으면 아무것도 하지 않는다.
pub fn delete_credentials(paths: &Paths) -> Result<()> {
    match fs::remove_file(paths.credentials_file()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).context("credentials 파일을 지우지 못했어요"),
    }
}

/// 디렉터리를 만들고(없으면) 권한을 0700으로 맞춘다.
pub fn ensure_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)
        .with_context(|| format!("{} 디렉터리를 만들지 못했어요", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// 파일을 0600 권한으로 쓴다.
pub fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(bytes)?;
        // 이미 있던 파일에는 mode가 적용되지 않으므로 다시 맞춘다
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes)?;
        Ok(())
    }
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test config::`
Expected: `test result: ok. 15 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add .gitignore Cargo.toml Cargo.lock src/lib.rs src/config.rs
git commit -m "feat(config): 경로·설정·API 키 저장" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 2: Linear 응답 타입 + 테스트용 이슈 빌더

Linear 스키마에서 `number`, `priority`, `estimate`, 사이클 `number`는 **Float**다. 정수 필드로 바로 받으면 `131.0` 같은 값에서 역직렬화가 실패한다. 그래서 f64로 받아 i64로 바꾼다.

**Files:**
- Create: `src/linear/mod.rs`, `src/linear/types.rs`, `src/test_support.rs`
- Modify: `src/lib.rs`
- Test: `src/linear/types.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 없음
- Produces:
  - `linear::types::Issue { id, identifier, number: i64, title, description: Option<String>, priority: i64, estimate: Option<f64>, url, branch_name, due_date: Option<String>, created_at, updated_at, archived_at: Option<String>, trashed: Option<bool>, team: TeamRef, state: StateRef, assignee: Option<UserRef>, project: Option<NamedRef>, cycle: Option<CycleRef>, parent: Option<ParentRef>, labels: Nodes<LabelRef> }`
  - `Issue::is_gone(&self) -> bool`(보관 또는 휴지통), `Issue::label_names(&self) -> Vec<&str>`
  - `Nodes<T> { nodes: Vec<T> }`, `TeamRef { id, key, name }`, `StateRef { id, name, state_type, color }`(JSON 필드명 `type`), `UserRef { id, name, display_name }`, `NamedRef { id, name }`, `CycleRef { id, number: f64, name: Option<String> }`, `ParentRef { id, identifier, title }`, `LabelRef { id, name, color }`
  - `Comment { id, body, created_at, edited_at: Option<String>, user: Option<UserRef> }`
  - `Viewer { id, name, display_name, email, organization: Organization, teams: Nodes<TeamRef> }`, `Organization { id, name, url_key }`
  - `PageInfo { has_next_page: bool, end_cursor: Option<String> }`, `IssuePage { nodes: Vec<Issue>, page_info: PageInfo }`
  - `IssueDetail { issue: Issue, comments: Vec<Comment>, more_comments: bool }`
  - 테스트 전용 `test_support::IssueBuilder`: `new(id, identifier, title)`, `.description(&str)`, `.state(name, type)`, `.priority(i64)`, `.updated(ts)`, `.assignee(id, name)`, `.labels(&[&str])`, `.archived()`, `.json() -> Value`, `.build() -> Issue`

- [ ] **Step 1: 모듈 선언, 테스트 빌더, 실패하는 테스트 작성**

`src/lib.rs` 전체:

````rust
//! herdr-linear: herdr 안에서 Linear를 빠르게 조회한다.

pub mod config;
pub mod linear;

#[cfg(test)]
pub mod test_support;
````

`src/linear/mod.rs`:

````rust
//! Linear GraphQL API.

pub mod types;
````

`src/test_support.rs`:

````rust
//! 테스트용 이슈 데이터.

use serde_json::{Value, json};

use crate::linear::types::Issue;

/// 필드를 바꿔 가며 테스트 이슈를 만든다.
pub struct IssueBuilder(Value);

impl IssueBuilder {
    /// `identifier`는 `KEY-번호` 형식이어야 한다.
    pub fn new(id: &str, identifier: &str, title: &str) -> Self {
        let (key, num) = identifier.split_once('-').expect("KEY-번호 형식");
        IssueBuilder(json!({
            "id": id,
            "identifier": identifier,
            "number": num.parse::<f64>().unwrap(),
            "title": title,
            "description": null,
            "priority": 0.0,
            "estimate": null,
            "url": format!("https://linear.app/acme/issue/{identifier}"),
            "branchName": format!("me/{}", identifier.to_lowercase()),
            "dueDate": null,
            "createdAt": "2026-10-01T00:00:00.000Z",
            "updatedAt": "2026-10-01T00:00:00.000Z",
            "archivedAt": null,
            "trashed": null,
            "team": { "id": format!("team-{key}"), "key": key, "name": key },
            "state": { "id": "st-unstarted", "name": "Todo", "type": "unstarted", "color": "#e2e2e2" },
            "assignee": null,
            "project": null,
            "cycle": null,
            "parent": null,
            "labels": { "nodes": [] }
        }))
    }

    pub fn description(mut self, d: &str) -> Self {
        self.0["description"] = json!(d);
        self
    }

    pub fn state(mut self, name: &str, state_type: &str) -> Self {
        self.0["state"] = json!({ "id": format!("st-{state_type}"), "name": name, "type": state_type, "color": "#5e6ad2" });
        self
    }

    pub fn priority(mut self, p: i64) -> Self {
        self.0["priority"] = json!(p as f64);
        self
    }

    pub fn updated(mut self, ts: &str) -> Self {
        self.0["updatedAt"] = json!(ts);
        self
    }

    pub fn assignee(mut self, id: &str, name: &str) -> Self {
        self.0["assignee"] = json!({ "id": id, "name": name, "displayName": name });
        self
    }

    pub fn labels(mut self, names: &[&str]) -> Self {
        let nodes: Vec<Value> = names
            .iter()
            .map(|n| json!({ "id": format!("lb-{n}"), "name": n, "color": "#eb5757" }))
            .collect();
        self.0["labels"] = json!({ "nodes": nodes });
        self
    }

    pub fn archived(mut self) -> Self {
        self.0["archivedAt"] = json!("2026-10-02T00:00:00.000Z");
        self
    }

    pub fn json(self) -> Value {
        self.0
    }

    pub fn build(self) -> Issue {
        serde_json::from_value(self.0).expect("유효한 이슈 JSON")
    }
}
````

`src/linear/types.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

    #[test]
    fn float_numbers_become_integers() {
        let mut v = IssueBuilder::new("i1", "ENG-131", "로그인 버그").json();
        v["number"] = serde_json::json!(131.0);
        v["priority"] = serde_json::json!(2.0);
        let issue: Issue = serde_json::from_value(v).unwrap();
        assert_eq!(issue.number, 131);
        assert_eq!(issue.priority, 2);
    }

    #[test]
    fn roundtrip_through_json_keeps_fields() {
        let issue = IssueBuilder::new("i1", "ENG-131", "로그인 버그")
            .labels(&["bug", "frontend"])
            .assignee("u1", "김민수")
            .build();
        let back: Issue = serde_json::from_str(&serde_json::to_string(&issue).unwrap()).unwrap();
        assert_eq!(back, issue);
        assert_eq!(back.label_names(), vec!["bug", "frontend"]);
    }

    #[test]
    fn archived_or_trashed_is_gone() {
        assert!(!IssueBuilder::new("i1", "ENG-1", "a").build().is_gone());
        assert!(
            IssueBuilder::new("i1", "ENG-1", "a")
                .archived()
                .build()
                .is_gone()
        );
        let mut v = IssueBuilder::new("i1", "ENG-1", "a").json();
        v["trashed"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Issue>(v).unwrap().is_gone());
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test linear::types`
Expected: 컴파일 실패 — `cannot find type `Issue`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/linear/types.rs` 맨 위에 넣는다:

````rust
//! Linear 응답 타입. 숫자 필드 중 Linear 스키마에서 Float인 것은 f64로 받는다.

use serde::{Deserialize, Deserializer, Serialize};

/// Linear는 `number`, `priority`를 Float으로 준다. 정수로 바꿔 쓴다.
fn f64_as_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(f64::deserialize(d)? as i64)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    #[serde(deserialize_with = "f64_as_i64")]
    pub number: i64,
    pub title: String,
    pub description: Option<String>,
    /// 0 없음, 1 긴급, 2 높음, 3 보통, 4 낮음
    #[serde(deserialize_with = "f64_as_i64")]
    pub priority: i64,
    pub estimate: Option<f64>,
    pub url: String,
    pub branch_name: String,
    pub due_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    #[serde(default)]
    pub trashed: Option<bool>,
    pub team: TeamRef,
    pub state: StateRef,
    pub assignee: Option<UserRef>,
    pub project: Option<NamedRef>,
    pub cycle: Option<CycleRef>,
    pub parent: Option<ParentRef>,
    pub labels: Nodes<LabelRef>,
}

impl Issue {
    /// 보관됐거나 휴지통에 있는 이슈인지.
    pub fn is_gone(&self) -> bool {
        self.archived_at.is_some() || self.trashed == Some(true)
    }

    pub fn label_names(&self) -> Vec<&str> {
        self.labels.nodes.iter().map(|l| l.name.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nodes<T> {
    pub nodes: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRef {
    pub id: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRef {
    pub id: String,
    pub name: String,
    /// triage, backlog, unstarted, started, completed, canceled, duplicate
    #[serde(rename = "type")]
    pub state_type: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRef {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CycleRef {
    pub id: String,
    pub number: f64,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentRef {
    pub id: String,
    pub identifier: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelRef {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub created_at: String,
    pub edited_at: Option<String>,
    pub user: Option<UserRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewer {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub email: String,
    pub organization: Organization,
    pub teams: Nodes<TeamRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub url_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuePage {
    pub nodes: Vec<Issue>,
    pub page_info: PageInfo,
}

/// 상세 화면용: 이슈 + 코멘트(오래된 것부터).
#[derive(Debug, Clone, PartialEq)]
pub struct IssueDetail {
    pub issue: Issue,
    pub comments: Vec<Comment>,
    /// 코멘트가 50개를 넘는지.
    pub more_comments: bool,
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test linear::types`
Expected: `test result: ok. 3 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/linear/mod.rs src/linear/types.rs src/test_support.rs
git commit -m "feat(linear): 응답 타입과 테스트 빌더" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 3: Linear HTTP 클라이언트 (오류 분류·한도 추적)

Linear는 한도 초과를 **HTTP 400 + `RATELIMITED` 코드**로 알린다(429가 아님). ureq 3는 기본적으로 4xx/5xx를 오류로 바꾸므로 `http_status_as_error(false)`로 꺼야 본문을 읽을 수 있다. 테스트는 mockito 가짜 서버로 한다.

**Files:**
- Create: `src/linear/client.rs`
- Modify: `src/linear/mod.rs`
- Test: `src/linear/client.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 없음
- Produces:
  - `client::LINEAR_ENDPOINT`, `client::AUTO_PAUSE_THRESHOLD: i64 = 50`
  - `client::ApiError::{RateLimited { reset_at_ms: Option<i64> }, Auth, GraphQl(String), Offline(String), Decode(String)}` (`thiserror`, `Clone + PartialEq`)
  - `client::RateLimit { requests_remaining: Option<i64>, requests_reset_ms: Option<i64>, complexity: Option<i64> }`, `RateLimit::should_pause_auto(&self, now_ms: i64) -> bool`
  - `client::LinearClient::new(api_key: impl Into<String>) -> LinearClient`
  - `LinearClient::with_endpoint(api_key, endpoint) -> LinearClient`(테스트용 주소)
  - `LinearClient::execute<T: DeserializeOwned>(&self, query: &str, variables: serde_json::Value) -> Result<T, ApiError>`(응답의 `data`를 `T`로)
  - `LinearClient::rate_limit(&self) -> RateLimit`(마지막 응답의 헤더)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/linear/mod.rs`:

````rust
//! Linear GraphQL API.

pub mod client;
pub mod types;
````

`src/linear/client.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    #[derive(Debug, Deserialize, PartialEq)]
    struct ViewerData {
        viewer: ViewerId,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct ViewerId {
        id: String,
    }

    fn client(server: &mockito::Server) -> LinearClient {
        LinearClient::with_endpoint("lin_api_test", format!("{}/graphql", server.url()))
    }

    #[test]
    fn sends_raw_key_and_returns_data() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/graphql")
            .match_header("authorization", "lin_api_test")
            .match_header("content-type", Matcher::Regex("application/json".into()))
            .match_body(Matcher::PartialJson(
                json!({ "query": "{ viewer { id } }" }),
            ))
            .with_status(200)
            .with_header("x-ratelimit-requests-remaining", "2499")
            .with_header("x-ratelimit-requests-reset", "1791290000000")
            .with_header("x-complexity", "12")
            .with_body(r#"{"data":{"viewer":{"id":"u1"}}}"#)
            .create();
        let c = client(&server);
        let got: ViewerData = c.execute("{ viewer { id } }", json!({})).unwrap();
        assert_eq!(got.viewer.id, "u1");
        assert_eq!(
            c.rate_limit(),
            RateLimit {
                requests_remaining: Some(2499),
                requests_reset_ms: Some(1_791_290_000_000),
                complexity: Some(12),
            }
        );
        m.assert();
    }

    #[test]
    fn rate_limited_is_http_400_with_code() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_header("x-ratelimit-requests-remaining", "0")
            .with_header("x-ratelimit-requests-reset", "1791290000000")
            .with_body(r#"{"errors":[{"message":"Rate limit exceeded","extensions":{"code":"RATELIMITED"}}]}"#)
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(
            err,
            ApiError::RateLimited {
                reset_at_ms: Some(1_791_290_000_000)
            }
        );
    }

    #[test]
    fn authentication_error_code_means_auth() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#)
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(err, ApiError::Auth);
    }

    #[test]
    fn plain_401_means_auth() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(401)
            .with_body("Unauthorized")
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(err, ApiError::Auth);
    }

    #[test]
    fn other_graphql_error_keeps_message() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Field 'x' doesn't exist","extensions":{"code":"GRAPHQL_VALIDATION_FAILED"}}]}"#)
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(err, ApiError::GraphQl("Field 'x' doesn't exist".into()));
    }

    #[test]
    fn server_error_is_offline() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(503)
            .with_body("busy")
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert!(matches!(err, ApiError::Offline(_)), "{err:?}");
    }

    #[test]
    fn unreachable_server_is_offline() {
        let c = LinearClient::with_endpoint("k", "http://127.0.0.1:9/graphql");
        let err = c.execute::<Value>("{ x }", json!({})).unwrap_err();
        assert!(matches!(err, ApiError::Offline(_)), "{err:?}");
    }

    #[test]
    fn invalid_json_is_decode_error() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(200)
            .with_body("<html>")
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert!(matches!(err, ApiError::Decode(_)), "{err:?}");
    }

    #[test]
    fn pause_only_when_low_and_before_reset() {
        let low = RateLimit {
            requests_remaining: Some(10),
            requests_reset_ms: Some(2_000),
            complexity: None,
        };
        assert!(low.should_pause_auto(1_000));
        assert!(!low.should_pause_auto(3_000));
        let plenty = RateLimit {
            requests_remaining: Some(500),
            requests_reset_ms: Some(2_000),
            complexity: None,
        };
        assert!(!plenty.should_pause_auto(1_000));
        assert!(!RateLimit::default().should_pause_auto(1_000));
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test linear::client`
Expected: 컴파일 실패 — `cannot find type `LinearClient`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/linear/client.rs` 맨 위에 넣는다:

````rust
//! Linear GraphQL HTTP 클라이언트: 요청 전송, 오류 분류, 한도 추적.

use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub const LINEAR_ENDPOINT: &str = "https://api.linear.app/graphql";

/// 남은 요청이 이보다 적으면 자동 요청(주기 새로고침·서버 검색)을 멈춘다.
pub const AUTO_PAUSE_THRESHOLD: i64 = 50;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("Linear API 한도를 넘었어요")]
    RateLimited { reset_at_ms: Option<i64> },
    #[error("API 키가 만료됐거나 권한이 없어요")]
    Auth,
    #[error("Linear 오류: {0}")]
    GraphQl(String),
    #[error("오프라인: {0}")]
    Offline(String),
    #[error("응답을 해석하지 못했어요: {0}")]
    Decode(String),
}

/// 마지막 응답의 한도 헤더.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RateLimit {
    pub requests_remaining: Option<i64>,
    /// UTC epoch 밀리초
    pub requests_reset_ms: Option<i64>,
    pub complexity: Option<i64>,
}

impl RateLimit {
    /// 자동 요청을 멈춰야 하는지. 리셋 시각이 지나면 다시 허용한다.
    pub fn should_pause_auto(&self, now_ms: i64) -> bool {
        match self.requests_remaining {
            Some(r) if r < AUTO_PAUSE_THRESHOLD => {
                self.requests_reset_ms.is_none_or(|reset| now_ms < reset)
            }
            _ => false,
        }
    }
}

pub struct LinearClient {
    agent: ureq::Agent,
    endpoint: String,
    api_key: String,
    rate: Mutex<RateLimit>,
}

impl LinearClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        LinearClient::with_endpoint(api_key, LINEAR_ENDPOINT)
    }

    /// 테스트에서 가짜 서버 주소를 넣을 때 쓴다.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .build()
            .into();
        LinearClient {
            agent,
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            rate: Mutex::new(RateLimit::default()),
        }
    }

    pub fn rate_limit(&self) -> RateLimit {
        *self.rate.lock().unwrap()
    }

    /// GraphQL 요청을 보내고 응답의 `data`를 `T`로 돌려준다.
    pub fn execute<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T, ApiError> {
        let body = json!({ "query": query, "variables": variables });
        let mut resp = self
            .agent
            .post(&self.endpoint)
            .header("Authorization", self.api_key.as_str())
            .send_json(&body)
            .map_err(|e| ApiError::Offline(e.to_string()))?;
        let status = resp.status().as_u16();
        let rate = read_rate_limit(resp.headers());
        if rate.requests_remaining.is_some() || rate.complexity.is_some() {
            *self.rate.lock().unwrap() = rate;
        }
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| ApiError::Offline(e.to_string()))?;
        parse_response(status, &text, rate.requests_reset_ms)
    }
}

fn read_rate_limit(headers: &ureq::http::HeaderMap) -> RateLimit {
    let num = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<i64>().ok())
    };
    RateLimit {
        requests_remaining: num("x-ratelimit-requests-remaining"),
        requests_reset_ms: num("x-ratelimit-requests-reset"),
        complexity: num("x-complexity"),
    }
}

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<Value>,
    #[serde(default)]
    errors: Vec<GqlError>,
}

#[derive(Deserialize)]
struct GqlError {
    #[serde(default)]
    message: String,
    #[serde(default)]
    extensions: Option<GqlExtensions>,
}

#[derive(Deserialize)]
struct GqlExtensions {
    code: Option<String>,
}

/// 응답을 해석한다. Linear는 한도 초과를 HTTP 400 + `RATELIMITED` 코드로 알린다.
fn parse_response<T: DeserializeOwned>(
    status: u16,
    text: &str,
    reset_ms: Option<i64>,
) -> Result<T, ApiError> {
    let resp: GqlResponse = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return Err(match status {
                401 | 403 => ApiError::Auth,
                429 => ApiError::RateLimited {
                    reset_at_ms: reset_ms,
                },
                s if s >= 500 => ApiError::Offline(format!("Linear 서버 오류 ({s})")),
                _ => ApiError::Decode(e.to_string()),
            });
        }
    };
    if !resp.errors.is_empty() {
        let has_code = |code: &str| {
            resp.errors
                .iter()
                .any(|e| e.extensions.as_ref().and_then(|x| x.code.as_deref()) == Some(code))
        };
        if has_code("RATELIMITED") {
            return Err(ApiError::RateLimited {
                reset_at_ms: reset_ms,
            });
        }
        if has_code("AUTHENTICATION_ERROR") || status == 401 {
            return Err(ApiError::Auth);
        }
        return Err(ApiError::GraphQl(resp.errors[0].message.clone()));
    }
    if status >= 500 {
        return Err(ApiError::Offline(format!("Linear 서버 오류 ({status})")));
    }
    if status == 401 || status == 403 {
        return Err(ApiError::Auth);
    }
    let data = resp
        .data
        .ok_or_else(|| ApiError::Decode("data가 없어요".to_string()))?;
    serde_json::from_value(data).map_err(|e| ApiError::Decode(e.to_string()))
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test linear::client`
Expected: `test result: ok. 9 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add src/linear/mod.rs src/linear/client.rs
git commit -m "feat(linear): HTTP 클라이언트와 오류·한도 처리" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 4: GraphQL 쿼리 함수

`searchIssues`의 결과 타입은 `Issue`가 아니라 `IssueSearchResult`라서, 같은 필드 목록으로 fragment를 하나 더 만든다. `issue(id:)`는 없는 이슈에 대해 GraphQL 오류("not found")를 돌려준다. 이 경우는 `Ok(None)`으로 바꾼다.

**Files:**
- Create: `src/linear/queries.rs`
- Modify: `src/linear/mod.rs`
- Test: `src/linear/queries.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `LinearClient::execute`, `ApiError`(Task 3), `types::*`(Task 2), `IssueBuilder`(테스트)
- Produces:
  - `queries::PAGE_SIZE: i64 = 50`, `DEEP_SEARCH_SIZE: i64 = 20`, `COMMENT_PAGE_SIZE: i64 = 50`
  - `queries::issue_fragment() -> String`(`fragment IssueFields on Issue { ... }`)
  - `queries::viewer(&LinearClient) -> Result<Viewer, ApiError>`
  - `queries::my_issues(&LinearClient) -> Result<Vec<Issue>, ApiError>`(나에게 할당 + 완료·취소·중복 제외)
  - `queries::team_issues(&LinearClient, team_ids: &[String], after: Option<&str>) -> Result<IssuePage, ApiError>`
  - `queries::filter_issues(&LinearClient, filter: &Value) -> Result<Vec<Issue>, ApiError>`
  - `queries::deep_search(&LinearClient, term: &str, filter: Option<&Value>) -> Result<Vec<Issue>, ApiError>`
  - `queries::issue_detail(&LinearClient, id: &str) -> Result<Option<IssueDetail>, ApiError>`(코멘트 오래된 순)
  - `queries::branch_issue(&LinearClient, branch: &str) -> Result<Option<Issue>, ApiError>`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/linear/mod.rs`:

````rust
//! Linear GraphQL API.

pub mod client;
pub mod queries;
pub mod types;
````

`src/linear/queries.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;

    fn client(server: &mockito::Server) -> LinearClient {
        LinearClient::with_endpoint("lin_api_test", format!("{}/graphql", server.url()))
    }

    #[test]
    fn viewer_parses_org_and_teams() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Viewer".into()))
            .with_body(
                json!({ "data": { "viewer": {
                    "id": "u1", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
                    "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
                    "teams": { "nodes": [ { "id": "team-ENG", "key": "ENG", "name": "Engineering" } ] }
                } } })
                .to_string(),
            )
            .create();
        let v = viewer(&client(&server)).unwrap();
        assert_eq!(v.organization.name, "Acme");
        assert_eq!(v.teams.nodes[0].key, "ENG");
    }

    #[test]
    fn my_issues_sends_filter_and_fragment() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/graphql")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("fragment IssueFields on Issue".into()),
                Matcher::Regex("orderBy: updatedAt".into()),
                Matcher::PartialJson(json!({ "variables": {
                    "first": 50,
                    "filter": {
                        "assignee": { "isMe": { "eq": true } },
                        "state": { "type": { "nin": ["completed", "canceled", "duplicate"] } }
                    }
                } })),
            ]))
            .with_body(
                json!({ "data": { "issues": {
                    "nodes": [ IssueBuilder::new("i1", "ENG-1", "첫 이슈").json() ],
                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                } } })
                .to_string(),
            )
            .create();
        let issues = my_issues(&client(&server)).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].identifier, "ENG-1");
        m.assert();
    }

    #[test]
    fn team_issues_pages_with_cursor() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::PartialJson(json!({ "variables": {
                "after": "cur1",
                "filter": { "team": { "id": { "in": ["team-ENG"] } } }
            } })))
            .with_body(
                json!({ "data": { "issues": {
                    "nodes": [ IssueBuilder::new("i2", "ENG-2", "둘째").json() ],
                    "pageInfo": { "hasNextPage": true, "endCursor": "cur2" }
                } } })
                .to_string(),
            )
            .create();
        let page = team_issues(&client(&server), &["team-ENG".to_string()], Some("cur1")).unwrap();
        assert!(page.page_info.has_next_page);
        assert_eq!(page.page_info.end_cursor.as_deref(), Some("cur2"));
    }

    #[test]
    fn deep_search_uses_search_fragment() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("fragment SearchFields on IssueSearchResult".into()),
                Matcher::Regex("includeComments: true".into()),
                Matcher::PartialJson(json!({ "variables": { "term": "세션", "first": 20 } })),
            ]))
            .with_body(
                json!({ "data": { "searchIssues": { "nodes": [ IssueBuilder::new("i3", "ENG-3", "세션 만료").json() ] } } })
                    .to_string(),
            )
            .create();
        let found = deep_search(&client(&server), "세션", None).unwrap();
        assert_eq!(found[0].identifier, "ENG-3");
    }

    #[test]
    fn detail_sorts_comments_oldest_first() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "상세").json();
        issue["comments"] = json!({
            "nodes": [
                { "id": "c2", "body": "두 번째", "createdAt": "2026-10-02T00:00:00.000Z", "editedAt": null, "user": null },
                { "id": "c1", "body": "첫 번째", "createdAt": "2026-10-01T00:00:00.000Z", "editedAt": null,
                  "user": { "id": "u1", "name": "김민수", "displayName": "minsu" } }
            ],
            "pageInfo": { "hasNextPage": true, "endCursor": "x" }
        });
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::PartialJson(
                json!({ "variables": { "id": "ENG-1", "first": 50 } }),
            ))
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let d = issue_detail(&client(&server), "ENG-1").unwrap().unwrap();
        assert_eq!(d.issue.identifier, "ENG-1");
        assert_eq!(
            d.comments.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["c1", "c2"]
        );
        assert!(d.more_comments);
    }

    #[test]
    fn detail_not_found_is_none() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"code":"INVALID_INPUT"}}]}"#)
            .create();
        assert_eq!(issue_detail(&client(&server), "ENG-404").unwrap(), None);
    }

    #[test]
    fn branch_issue_can_be_missing() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::PartialJson(
                json!({ "variables": { "branch": "main" } }),
            ))
            .with_body(r#"{"data":{"issueVcsBranchSearch":null}}"#)
            .create();
        assert_eq!(branch_issue(&client(&server), "main").unwrap(), None);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test linear::queries`
Expected: 컴파일 실패 — `cannot find function `viewer`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/linear/queries.rs` 맨 위에 넣는다:

````rust
//! GraphQL 문서와 호출 함수.

use serde::Deserialize;
use serde_json::{Value, json};

use super::client::{ApiError, LinearClient};
use super::types::{Comment, Issue, IssueDetail, IssuePage, Nodes, PageInfo, Viewer};

pub const PAGE_SIZE: i64 = 50;
pub const DEEP_SEARCH_SIZE: i64 = 20;
pub const COMMENT_PAGE_SIZE: i64 = 50;

/// 이슈 하나에 대해 가져오는 필드. `Issue`와 `IssueSearchResult`에 똑같이 쓴다.
const ISSUE_SELECTION: &str = "id identifier number title description priority estimate url branchName dueDate createdAt updatedAt archivedAt trashed team { id key name } state { id name type color } assignee { id name displayName } project { id name } cycle { id number name } parent { id identifier title } labels(first: 20) { nodes { id name color } }";

pub fn issue_fragment() -> String {
    format!("fragment IssueFields on Issue {{ {ISSUE_SELECTION} }}")
}

fn search_fragment() -> String {
    format!("fragment SearchFields on IssueSearchResult {{ {ISSUE_SELECTION} }}")
}

/// 키 검증과 내 정보.
pub fn viewer(c: &LinearClient) -> Result<Viewer, ApiError> {
    #[derive(Deserialize)]
    struct D {
        viewer: Viewer,
    }
    let q = "query Viewer { viewer { id name displayName email organization { id name urlKey } teams { nodes { id key name } } } }";
    Ok(c.execute::<D>(q, json!({}))?.viewer)
}

/// 나에게 할당된 열린 이슈 (완료·취소·중복 제외).
pub fn my_issues(c: &LinearClient) -> Result<Vec<Issue>, ApiError> {
    let filter = json!({
        "assignee": { "isMe": { "eq": true } },
        "state": { "type": { "nin": ["completed", "canceled", "duplicate"] } }
    });
    Ok(issues_page(c, &filter, None)?.nodes)
}

/// 범위 팀의 이슈를 최근 수정 순으로 한 페이지씩.
pub fn team_issues(
    c: &LinearClient,
    team_ids: &[String],
    after: Option<&str>,
) -> Result<IssuePage, ApiError> {
    let filter = json!({ "team": { "id": { "in": team_ids } } });
    issues_page(c, &filter, after)
}

/// 필터 검색 (첫 50건).
pub fn filter_issues(c: &LinearClient, filter: &Value) -> Result<Vec<Issue>, ApiError> {
    Ok(issues_page(c, filter, None)?.nodes)
}

fn issues_page(
    c: &LinearClient,
    filter: &Value,
    after: Option<&str>,
) -> Result<IssuePage, ApiError> {
    #[derive(Deserialize)]
    struct D {
        issues: IssuePage,
    }
    let q = format!(
        "query Issues($filter: IssueFilter, $after: String, $first: Int) {{ issues(filter: $filter, after: $after, first: $first, orderBy: updatedAt) {{ nodes {{ ...IssueFields }} pageInfo {{ hasNextPage endCursor }} }} }} {}",
        issue_fragment()
    );
    let vars = json!({ "filter": filter, "after": after, "first": PAGE_SIZE });
    Ok(c.execute::<D>(&q, vars)?.issues)
}

/// 서버 깊은 검색 (코멘트 포함). 분당 30회 제한이 있으니 사용자가 고를 때만 부른다.
pub fn deep_search(
    c: &LinearClient,
    term: &str,
    filter: Option<&Value>,
) -> Result<Vec<Issue>, ApiError> {
    #[derive(Deserialize)]
    struct D {
        #[serde(rename = "searchIssues")]
        search: Nodes<Issue>,
    }
    let q = format!(
        "query Deep($term: String!, $filter: IssueFilter, $first: Int) {{ searchIssues(term: $term, filter: $filter, first: $first, includeComments: true) {{ nodes {{ ...SearchFields }} }} }} {}",
        search_fragment()
    );
    let vars = json!({ "term": term, "filter": filter, "first": DEEP_SEARCH_SIZE });
    Ok(c.execute::<D>(&q, vars)?.search.nodes)
}

/// 이슈와 코멘트. 이슈가 없으면 `None`.
pub fn issue_detail(c: &LinearClient, id: &str) -> Result<Option<IssueDetail>, ApiError> {
    #[derive(Deserialize)]
    struct D {
        issue: DetailIssue,
    }
    #[derive(Deserialize)]
    struct DetailIssue {
        #[serde(flatten)]
        issue: Issue,
        comments: CommentPage,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CommentPage {
        nodes: Vec<Comment>,
        page_info: PageInfo,
    }
    let q = format!(
        "query Detail($id: String!, $first: Int) {{ issue(id: $id) {{ ...IssueFields comments(first: $first) {{ nodes {{ id body createdAt editedAt user {{ id name displayName }} }} pageInfo {{ hasNextPage endCursor }} }} }} }} {}",
        issue_fragment()
    );
    match c.execute::<D>(&q, json!({ "id": id, "first": COMMENT_PAGE_SIZE })) {
        Ok(d) => {
            let mut comments = d.issue.comments.nodes;
            comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            Ok(Some(IssueDetail {
                issue: d.issue.issue,
                comments,
                more_comments: d.issue.comments.page_info.has_next_page,
            }))
        }
        Err(ApiError::GraphQl(msg)) if is_not_found(&msg) => Ok(None),
        Err(e) => Err(e),
    }
}

fn is_not_found(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("not found") || m.contains("could not find")
}

/// 브랜치명으로 연결된 이슈를 찾는다.
pub fn branch_issue(c: &LinearClient, branch: &str) -> Result<Option<Issue>, ApiError> {
    #[derive(Deserialize)]
    struct D {
        #[serde(rename = "issueVcsBranchSearch")]
        issue: Option<Issue>,
    }
    let q = format!(
        "query Branch($branch: String!) {{ issueVcsBranchSearch(branchName: $branch) {{ ...IssueFields }} }} {}",
        issue_fragment()
    );
    Ok(c.execute::<D>(&q, json!({ "branch": branch }))?.issue)
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test linear::queries`
Expected: `test result: ok. 7 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add src/linear/mod.rs src/linear/queries.rs
git commit -m "feat(linear): 조회 쿼리 함수" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 5: 검색어 해석

스펙 4.5와 7.3장의 토큰 규칙이다.
- 따옴표 안의 공백은 유지한다.
- `ABC-123`은 식별자, 숫자만 있는 단어는 이슈 번호로 해석한다.
- 같은 종류의 토큰이 여러 번 나오면 마지막 것을 쓴다.
- 알 수 없는 `p:` 값은 무시한다.

**Files:**
- Create: `src/search/mod.rs`, `src/search/query.rs`
- Modify: `src/lib.rs`
- Test: `src/search/query.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 없음
- Produces:
  - `query::ParsedQuery { words: Vec<String>, number: Option<i64>, identifier: Option<(String, i64)>, state: Option<StateMatch>, label: Option<String>, assignee: Option<AssigneeMatch>, team: Option<String>, priority: Option<i64> }` (`Default + PartialEq`)
  - `ParsedQuery::has_text(&self) -> bool`, `is_empty(&self) -> bool`, `text(&self) -> String`(깊은 검색어)
  - `query::StateMatch::{Type(&'static str), Name(String)}`, `query::AssigneeMatch::{Me, Name(String)}`
  - `query::parse(&str) -> ParsedQuery`
  - `query::parse_identifier(&str) -> Option<(String, i64)>`, `state_type_keyword(&str) -> Option<&'static str>`, `priority_keyword(&str) -> Option<i64>`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/lib.rs` 전체:

````rust
//! herdr-linear: herdr 안에서 Linear를 빠르게 조회한다.

pub mod config;
pub mod linear;
pub mod search;

#[cfg(test)]
pub mod test_support;
````

`src/search/mod.rs`:

````rust
//! 검색어 해석과 로컬 순위.

pub mod query;
````

`src/search/query.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_text_words() {
        let q = parse("로그인  버그");
        assert_eq!(q.words, vec!["로그인", "버그"]);
        assert!(q.has_text());
    }

    #[test]
    fn all_tokens_together() {
        let q = parse("로그인 l:bug s:진행 @나 #eng p:high");
        assert_eq!(q.words, vec!["로그인"]);
        assert_eq!(q.label.as_deref(), Some("bug"));
        assert_eq!(q.state, Some(StateMatch::Type("started")));
        assert_eq!(q.assignee, Some(AssigneeMatch::Me));
        assert_eq!(q.team.as_deref(), Some("ENG"));
        assert_eq!(q.priority, Some(2));
    }

    #[test]
    fn quoted_values_keep_spaces() {
        let q = parse("s:\"In Review\" \"로그인 버튼\"");
        assert_eq!(q.state, Some(StateMatch::Name("In Review".into())));
        assert_eq!(q.words, vec!["로그인 버튼"]);
    }

    #[test]
    fn identifier_and_number() {
        let q = parse("eng-131 42");
        assert_eq!(q.identifier, Some(("ENG".into(), 131)));
        assert_eq!(q.number, Some(42));
        assert_eq!(q.text(), "ENG-131 42");
    }

    #[test]
    fn prefixes_are_case_insensitive_and_names_kept() {
        let q = parse("S:done L:Frontend @민수 P:긴급");
        assert_eq!(q.state, Some(StateMatch::Type("completed")));
        assert_eq!(q.label.as_deref(), Some("Frontend"));
        assert_eq!(q.assignee, Some(AssigneeMatch::Name("민수".into())));
        assert_eq!(q.priority, Some(1));
    }

    #[test]
    fn unknown_priority_and_empty_tokens_are_ignored() {
        let q = parse("p:soon l: s: @ #");
        assert_eq!(q.priority, None);
        assert_eq!(q.label, None);
        assert_eq!(q.state, None);
        assert_eq!(q.words, vec!["@", "#"]);
    }

    #[test]
    fn decomposed_korean_is_normalized() {
        let nfd: String = "로그인".nfd().collect();
        assert_eq!(parse(&nfd).words, vec!["로그인"]);
    }

    #[test]
    fn empty_query() {
        assert!(parse("   ").is_empty());
        assert!(!parse("l:bug").has_text());
    }

    #[test]
    fn identifier_rules() {
        assert_eq!(parse_identifier("ABC-12"), Some(("ABC".into(), 12)));
        assert_eq!(parse_identifier("a1-3"), Some(("A1".into(), 3)));
        assert_eq!(parse_identifier("1A-3"), None);
        assert_eq!(parse_identifier("ABC-"), None);
        assert_eq!(parse_identifier("ABC-1x"), None);
        assert_eq!(parse_identifier("로그-1"), None);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test search::query`
Expected: 컴파일 실패 — `cannot find function `parse`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/search/query.rs` 맨 위에 넣는다:

````rust
//! 검색어 해석: 자유 텍스트 + 필터 토큰(s: l: @ # p:).

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateMatch {
    /// 상태 타입 (triage, backlog, unstarted, started, completed, canceled)
    Type(&'static str),
    /// 상태 이름 일부
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssigneeMatch {
    Me,
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedQuery {
    /// 자유 텍스트 단어 (따옴표로 묶으면 공백 포함 한 단어)
    pub words: Vec<String>,
    /// 숫자만 있는 단어 (이슈 번호)
    pub number: Option<i64>,
    /// `ABC-123` 형식 (팀 키는 대문자)
    pub identifier: Option<(String, i64)>,
    pub state: Option<StateMatch>,
    pub label: Option<String>,
    pub assignee: Option<AssigneeMatch>,
    /// 팀 키 (대문자)
    pub team: Option<String>,
    /// 0 없음 … 4 낮음
    pub priority: Option<i64>,
}

impl ParsedQuery {
    /// 텍스트 조건(단어·번호·식별자)이 있는지.
    pub fn has_text(&self) -> bool {
        !self.words.is_empty() || self.number.is_some() || self.identifier.is_some()
    }

    pub fn is_empty(&self) -> bool {
        *self == ParsedQuery::default()
    }

    /// 깊은 검색에 보낼 텍스트.
    pub fn text(&self) -> String {
        let mut parts = self.words.clone();
        if let Some((key, n)) = &self.identifier {
            parts.push(format!("{key}-{n}"));
        }
        if let Some(n) = self.number {
            parts.push(n.to_string());
        }
        parts.join(" ")
    }
}

/// 검색어를 해석한다. 같은 종류의 토큰이 여러 번 나오면 마지막 것을 쓴다.
pub fn parse(input: &str) -> ParsedQuery {
    let normalized: String = input.nfc().collect();
    let mut q = ParsedQuery::default();
    for token in tokenize(&normalized) {
        if let Some(v) = strip_prefix_ci(&token, "s:") {
            if let Some(t) = state_type_keyword(v) {
                q.state = Some(StateMatch::Type(t));
            } else if !v.is_empty() {
                q.state = Some(StateMatch::Name(v.to_string()));
            }
        } else if let Some(v) = strip_prefix_ci(&token, "l:") {
            if !v.is_empty() {
                q.label = Some(v.to_string());
            }
        } else if let Some(v) = strip_prefix_ci(&token, "p:") {
            // 알 수 없는 우선순위 값은 무시한다
            if let Some(p) = priority_keyword(v) {
                q.priority = Some(p);
            }
        } else if let Some(v) = token.strip_prefix('@').filter(|v| !v.is_empty()) {
            let lower = v.to_lowercase();
            q.assignee = Some(if lower == "나" || lower == "me" {
                AssigneeMatch::Me
            } else {
                AssigneeMatch::Name(v.to_string())
            });
        } else if let Some(v) = token.strip_prefix('#').filter(|v| !v.is_empty()) {
            q.team = Some(v.to_uppercase());
        } else if let Some(id) = parse_identifier(&token) {
            q.identifier = Some(id);
        } else if token.chars().all(|c| c.is_ascii_digit()) {
            match token.parse::<i64>() {
                Ok(n) => q.number = Some(n),
                Err(_) => q.words.push(token),
            }
        } else {
            q.words.push(token);
        }
    }
    q
}

/// 공백으로 나누되 큰따옴표 안의 공백은 유지한다. 따옴표 자체는 지운다.
fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in s.chars() {
        match c {
            '"' => in_quote = !in_quote,
            c if c.is_whitespace() && !in_quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let n = prefix.len();
    (s.len() >= n && s.is_char_boundary(n) && s[..n].eq_ignore_ascii_case(prefix)).then(|| &s[n..])
}

/// `ABC-123` 형식이면 (팀 키 대문자, 번호).
pub fn parse_identifier(s: &str) -> Option<(String, i64)> {
    let (key, num) = s.split_once('-')?;
    let mut chars = key.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() || !chars.all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((key.to_uppercase(), num.parse().ok()?))
}

/// 상태 타입 키워드 (한글 포함).
pub fn state_type_keyword(v: &str) -> Option<&'static str> {
    match v.to_lowercase().as_str() {
        "triage" | "분류" => Some("triage"),
        "backlog" | "백로그" => Some("backlog"),
        "unstarted" | "todo" | "할일" => Some("unstarted"),
        "started" | "progress" | "진행" => Some("started"),
        "completed" | "done" | "완료" => Some("completed"),
        "canceled" | "취소" => Some("canceled"),
        _ => None,
    }
}

/// 우선순위 키워드 → 0~4.
pub fn priority_keyword(v: &str) -> Option<i64> {
    match v.to_lowercase().as_str() {
        "none" | "없음" | "0" => Some(0),
        "urgent" | "긴급" | "1" => Some(1),
        "high" | "높음" | "2" => Some(2),
        "medium" | "보통" | "3" => Some(3),
        "low" | "낮음" | "4" => Some(4),
        _ => None,
    }
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test search::query`
Expected: `test result: ok. 9 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/search/mod.rs src/search/query.rs
git commit -m "feat(search): 검색어 토큰 해석" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 6: 검색어 → IssueFilter 변환

스펙 7.3장의 변환표를 그대로 구현한다.
- `ABC-123`은 "팀 키 + 번호"와 "텍스트 포함"을 OR로 묶는다. `covid-19`처럼 팀이 없는 단어도 찾기 위해서다.
- 범위 팀 조건은 `#KEY` 토큰이 없을 때만 붙인다.

**Files:**
- Create: `src/linear/filter.rs`
- Modify: `src/linear/mod.rs`
- Test: `src/linear/filter.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `search::query::{ParsedQuery, StateMatch, AssigneeMatch, parse}`(Task 5)
- Produces:
  - `filter::build_issue_filter(&ParsedQuery, scope_team_ids: &[String]) -> serde_json::Value`
  - `filter::token_filter(&ParsedQuery) -> Option<serde_json::Value>`(깊은 검색용, 토큰 조건만)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/linear/mod.rs`:

````rust
//! Linear GraphQL API.

pub mod client;
pub mod filter;
pub mod queries;
pub mod types;
````

`src/linear/filter.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::query::parse;

    #[test]
    fn empty_query_without_scope_is_empty_filter() {
        assert_eq!(build_issue_filter(&parse(""), &[]), json!({}));
    }

    #[test]
    fn scope_only() {
        assert_eq!(
            build_issue_filter(&parse(""), &["t1".to_string()]),
            json!({ "team": { "id": { "in": ["t1"] } } })
        );
    }

    #[test]
    fn each_word_matches_title_or_description() {
        assert_eq!(
            build_issue_filter(&parse("로그인 버그"), &[]),
            json!({ "and": [
                { "or": [ { "title": { "containsIgnoreCase": "로그인" } }, { "description": { "containsIgnoreCase": "로그인" } } ] },
                { "or": [ { "title": { "containsIgnoreCase": "버그" } }, { "description": { "containsIgnoreCase": "버그" } } ] }
            ] })
        );
    }

    #[test]
    fn number_also_matches_issue_number() {
        assert_eq!(
            build_issue_filter(&parse("131"), &[]),
            json!({ "or": [
                { "title": { "containsIgnoreCase": "131" } },
                { "description": { "containsIgnoreCase": "131" } },
                { "number": { "eq": 131 } }
            ] })
        );
    }

    #[test]
    fn identifier_matches_team_and_number_or_text() {
        assert_eq!(
            build_issue_filter(&parse("eng-7"), &[]),
            json!({ "or": [
                { "team": { "key": { "eqIgnoreCase": "ENG" } }, "number": { "eq": 7 } },
                { "title": { "containsIgnoreCase": "ENG-7" } },
                { "description": { "containsIgnoreCase": "ENG-7" } }
            ] })
        );
    }

    #[test]
    fn tokens_map_to_filters_and_team_overrides_scope() {
        assert_eq!(
            build_issue_filter(
                &parse("s:진행 l:bug @나 #OPS p:urgent"),
                &["t1".to_string()]
            ),
            json!({ "and": [
                { "state": { "type": { "eq": "started" } } },
                { "labels": { "some": { "name": { "containsIgnoreCase": "bug" } } } },
                { "assignee": { "isMe": { "eq": true } } },
                { "team": { "key": { "eqIgnoreCase": "OPS" } } },
                { "priority": { "eq": 1 } }
            ] })
        );
    }

    #[test]
    fn state_name_and_assignee_name() {
        assert_eq!(
            build_issue_filter(&parse("s:\"In Review\" @민수"), &[]),
            json!({ "and": [
                { "state": { "name": { "containsIgnoreCase": "In Review" } } },
                { "assignee": { "or": [
                    { "name": { "containsIgnoreCase": "민수" } },
                    { "displayName": { "containsIgnoreCase": "민수" } }
                ] } }
            ] })
        );
    }

    #[test]
    fn token_filter_ignores_text() {
        assert_eq!(token_filter(&parse("세션 만료")), None);
        assert_eq!(
            token_filter(&parse("세션 l:bug")),
            Some(json!({ "labels": { "some": { "name": { "containsIgnoreCase": "bug" } } } }))
        );
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test linear::filter`
Expected: 컴파일 실패 — `cannot find function `build_issue_filter`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/linear/filter.rs` 맨 위에 넣는다:

````rust
//! 검색어 → Linear `IssueFilter` JSON.

use serde_json::{Value, json};

use crate::search::query::{AssigneeMatch, ParsedQuery, StateMatch};

/// 서버 필터 검색용. `scope_team_ids`는 `#KEY` 토큰이 없을 때만 적용한다.
pub fn build_issue_filter(q: &ParsedQuery, scope_team_ids: &[String]) -> Value {
    let mut parts = text_parts(q);
    parts.extend(token_parts(q));
    if q.team.is_none() && !scope_team_ids.is_empty() {
        parts.push(json!({ "team": { "id": { "in": scope_team_ids } } }));
    }
    combine(parts)
}

/// 깊은 검색(`searchIssues`)용: 토큰 조건만. 없으면 `None`.
pub fn token_filter(q: &ParsedQuery) -> Option<Value> {
    let parts = token_parts(q);
    (!parts.is_empty()).then(|| combine(parts))
}

fn combine(parts: Vec<Value>) -> Value {
    match parts.len() {
        0 => json!({}),
        1 => parts.into_iter().next().unwrap(),
        _ => json!({ "and": parts }),
    }
}

/// 제목 또는 본문에 포함.
fn contains_word(w: &str) -> Vec<Value> {
    vec![
        json!({ "title": { "containsIgnoreCase": w } }),
        json!({ "description": { "containsIgnoreCase": w } }),
    ]
}

fn text_parts(q: &ParsedQuery) -> Vec<Value> {
    let mut parts: Vec<Value> = q
        .words
        .iter()
        .map(|w| json!({ "or": contains_word(w) }))
        .collect();
    if let Some(n) = q.number {
        let mut alts = contains_word(&n.to_string());
        alts.push(json!({ "number": { "eq": n } }));
        parts.push(json!({ "or": alts }));
    }
    if let Some((key, n)) = &q.identifier {
        // 팀 키 + 번호로 찾고, 그런 팀이 없을 때를 위해 텍스트로도 찾는다 (예: covid-19)
        let mut alts =
            vec![json!({ "team": { "key": { "eqIgnoreCase": key } }, "number": { "eq": n } })];
        alts.extend(contains_word(&format!("{key}-{n}")));
        parts.push(json!({ "or": alts }));
    }
    parts
}

fn token_parts(q: &ParsedQuery) -> Vec<Value> {
    let mut parts = Vec::new();
    match &q.state {
        Some(StateMatch::Type(t)) => parts.push(json!({ "state": { "type": { "eq": t } } })),
        Some(StateMatch::Name(n)) => {
            parts.push(json!({ "state": { "name": { "containsIgnoreCase": n } } }))
        }
        None => {}
    }
    if let Some(l) = &q.label {
        parts.push(json!({ "labels": { "some": { "name": { "containsIgnoreCase": l } } } }));
    }
    match &q.assignee {
        Some(AssigneeMatch::Me) => parts.push(json!({ "assignee": { "isMe": { "eq": true } } })),
        Some(AssigneeMatch::Name(n)) => parts.push(json!({ "assignee": { "or": [
            { "name": { "containsIgnoreCase": n } },
            { "displayName": { "containsIgnoreCase": n } }
        ] } })),
        None => {}
    }
    if let Some(k) = &q.team {
        parts.push(json!({ "team": { "key": { "eqIgnoreCase": k } } }));
    }
    if let Some(p) = q.priority {
        parts.push(json!({ "priority": { "eq": p } }));
    }
    parts
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test linear::filter`
Expected: `test result: ok. 8 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add src/linear/mod.rs src/linear/filter.rs
git commit -m "feat(linear): 검색어를 IssueFilter로 변환" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 7: SQLite 캐시 저장소

스펙 6.2~6.3장이다.
- 보관·휴지통 이슈는 저장하지 않고 지운다.
- `viewed_at`은 덮어써도 유지한다.
- 30일 지난 이슈를 정리한다.
- 워크스페이스가 바뀌면 캐시를 비운다.
- **손상(NOTADB·CORRUPT)일 때만** 파일을 지우고 새로 만든다. 잠금 같은 일시 오류로 캐시를 지우면 안 된다(Review Focus).

**Files:**
- Create: `src/store/mod.rs`, `src/store/cache.rs`
- Modify: `src/lib.rs`
- Test: `src/store/mod.rs`, `src/store/cache.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `config::ensure_private_dir`(Task 1), `types::{Issue, Comment}`(Task 2), `IssueBuilder`(테스트)
- Produces:
  - `store::SCHEMA_VERSION: i64 = 1`, `store::Store`(필드 `conn`은 `pub(crate)`)
  - `Store::open(&Path) -> Result<Store>`, `Store::open_in_memory() -> Result<Store>`, `store::remove_db_files(&Path)`
  - `Store::meta_get(&str) -> Result<Option<String>>`, `meta_set(&str, &str) -> Result<()>`
  - `ensure_org(&str) -> Result<bool>`(비웠으면 true), `clear_all() -> Result<()>`
  - `upsert_issues(&[Issue], now_ms: i64) -> Result<Vec<String>>`(지운 id), `get_issue(id_or_identifier: &str) -> Result<Option<Issue>>`, `remove_issue(&str) -> Result<()>`, `all_issues() -> Result<Vec<Issue>>`
  - `set_view(key: &str, ids: &[String], now_ms) -> Result<()>`, `get_view(&str) -> Result<Option<(Vec<Issue>, i64)>>`
  - `set_comments(issue_id, &[Comment], now_ms) -> Result<()>`, `get_comments(&str) -> Result<Option<(Vec<Comment>, i64)>>`
  - `mark_viewed(&str, now_ms) -> Result<()>`, `recent_viewed(limit: usize) -> Result<Vec<Issue>>`, `evict_older_than(cutoff_ms: i64) -> Result<usize>`
  - `branch_get(repo, branch) -> Result<Option<(Option<String>, i64)>>`, `branch_set(repo, branch, Option<&str>, now_ms) -> Result<()>`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/lib.rs` 전체:

````rust
//! herdr-linear: herdr 안에서 Linear를 빠르게 조회한다.

pub mod config;
pub mod linear;
pub mod search;
pub mod store;

#[cfg(test)]
pub mod test_support;
````

`src/store/mod.rs` (`mod cache;`와 테스트 모듈만 먼저):

````rust
mod cache;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_schema_with_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("cache.db");
        let store = Store::open(&path).unwrap();
        let v: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        store.meta_set("k", "v").unwrap();
        drop(store);
        let again = Store::open(&path).unwrap();
        assert_eq!(again.meta_get("k").unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn version_mismatch_recreates_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        {
            let store = Store::open(&path).unwrap();
            store.meta_set("k", "v").unwrap();
            store.conn.pragma_update(None, "user_version", 99).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.meta_get("k").unwrap(), None);
    }

    #[test]
    fn corrupt_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        std::fs::write(
            &path,
            b"this is not a sqlite database at all, just junk bytes",
        )
        .unwrap();
        let store = Store::open(&path).unwrap();
        store.meta_set("k", "v").unwrap();
        assert_eq!(store.meta_get("k").unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn only_corruption_triggers_recreate() {
        use rusqlite::ffi;
        let busy = rusqlite::Error::SqliteFailure(ffi::Error::new(ffi::SQLITE_BUSY), None);
        assert!(!is_corrupt(&anyhow::Error::from(busy)));
        let not_db = rusqlite::Error::SqliteFailure(ffi::Error::new(ffi::SQLITE_NOTADB), None);
        assert!(is_corrupt(&anyhow::Error::from(not_db)));
    }

    #[test]
    fn two_writers_wait_instead_of_failing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        let a = Store::open(&path).unwrap();
        a.meta_set("keep", "yes").unwrap();
        // a가 쓰기 잠금을 잡은 동안 b가 열고 쓰면, 파일을 지우지 않고 기다렸다가 성공해야 한다
        a.conn
            .execute_batch("BEGIN IMMEDIATE; INSERT INTO meta (key, value) VALUES ('a', '1');")
            .unwrap();
        let b_path = path.clone();
        let writer = std::thread::spawn(move || {
            let b = Store::open(&b_path).unwrap();
            b.meta_set("b", "2").unwrap();
        });
        std::thread::sleep(Duration::from_millis(300));
        a.conn.execute_batch("COMMIT").unwrap();
        writer.join().unwrap();
        assert_eq!(a.meta_get("keep").unwrap().as_deref(), Some("yes"));
        assert_eq!(a.meta_get("b").unwrap().as_deref(), Some("2"));
    }

    #[cfg(unix)]
    #[test]
    fn db_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("cache.db");
        let _store = Store::open(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
    }
}
````

`src/store/cache.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn upsert_and_get_by_id_or_identifier() {
        let s = store();
        let issue = IssueBuilder::new("i1", "ENG-1", "로그인").build();
        s.upsert_issues(std::slice::from_ref(&issue), 100).unwrap();
        assert_eq!(s.get_issue("i1").unwrap(), Some(issue.clone()));
        assert_eq!(s.get_issue("eng-1").unwrap(), Some(issue));
        assert_eq!(s.get_issue("ENG-2").unwrap(), None);
    }

    #[test]
    fn upsert_keeps_viewed_at_and_updates_fields() {
        let s = store();
        s.upsert_issues(&[IssueBuilder::new("i1", "ENG-1", "옛 제목").build()], 100)
            .unwrap();
        s.mark_viewed("i1", 150).unwrap();
        s.upsert_issues(&[IssueBuilder::new("i1", "ENG-1", "새 제목").build()], 200)
            .unwrap();
        assert_eq!(s.get_issue("i1").unwrap().unwrap().title, "새 제목");
        assert_eq!(s.recent_viewed(10).unwrap().len(), 1);
    }

    #[test]
    fn archived_issue_is_removed_not_saved() {
        let s = store();
        s.upsert_issues(&[IssueBuilder::new("i1", "ENG-1", "a").build()], 100)
            .unwrap();
        s.set_comments("i1", &[], 100).unwrap();
        let removed = s
            .upsert_issues(
                &[IssueBuilder::new("i1", "ENG-1", "a").archived().build()],
                200,
            )
            .unwrap();
        assert_eq!(removed, vec!["i1".to_string()]);
        assert_eq!(s.get_issue("i1").unwrap(), None);
        assert_eq!(s.get_comments("i1").unwrap(), None);
    }

    #[test]
    fn identifier_moved_to_new_id_replaces_old_row() {
        let s = store();
        s.upsert_issues(&[IssueBuilder::new("old", "ENG-1", "a").build()], 100)
            .unwrap();
        s.upsert_issues(&[IssueBuilder::new("new", "ENG-1", "b").build()], 200)
            .unwrap();
        assert_eq!(s.get_issue("old").unwrap(), None);
        assert_eq!(s.get_issue("ENG-1").unwrap().unwrap().id, "new");
    }

    #[test]
    fn view_keeps_order_and_skips_missing() {
        let s = store();
        s.upsert_issues(
            &[
                IssueBuilder::new("a", "ENG-1", "a").build(),
                IssueBuilder::new("b", "ENG-2", "b").build(),
            ],
            100,
        )
        .unwrap();
        s.set_view("mine", &["b".into(), "gone".into(), "a".into()], 300)
            .unwrap();
        let (issues, at) = s.get_view("mine").unwrap().unwrap();
        assert_eq!(
            issues.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert_eq!(at, 300);
        assert_eq!(s.get_view("all").unwrap(), None);
    }

    #[test]
    fn comments_roundtrip() {
        let s = store();
        let c = Comment {
            id: "c1".into(),
            body: "확인할게요".into(),
            created_at: "2026-10-01T00:00:00.000Z".into(),
            edited_at: None,
            user: None,
        };
        s.set_comments("i1", std::slice::from_ref(&c), 500).unwrap();
        assert_eq!(s.get_comments("i1").unwrap(), Some((vec![c], 500)));
    }

    #[test]
    fn recent_viewed_is_newest_first_with_limit() {
        let s = store();
        s.upsert_issues(
            &[
                IssueBuilder::new("a", "ENG-1", "a").build(),
                IssueBuilder::new("b", "ENG-2", "b").build(),
                IssueBuilder::new("c", "ENG-3", "c").build(),
            ],
            100,
        )
        .unwrap();
        s.mark_viewed("a", 1).unwrap();
        s.mark_viewed("c", 3).unwrap();
        s.mark_viewed("b", 2).unwrap();
        let ids: Vec<String> = s
            .recent_viewed(2)
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, vec!["c", "b"]);
    }

    #[test]
    fn eviction_uses_latest_of_fetched_and_viewed() {
        let s = store();
        s.upsert_issues(
            &[
                IssueBuilder::new("old", "ENG-1", "a").build(),
                IssueBuilder::new("seen", "ENG-2", "b").build(),
            ],
            100,
        )
        .unwrap();
        s.set_comments("old", &[], 100).unwrap();
        s.mark_viewed("seen", 1_000).unwrap();
        assert_eq!(s.evict_older_than(500).unwrap(), 1);
        assert_eq!(s.get_issue("old").unwrap(), None);
        assert_eq!(s.get_comments("old").unwrap(), None);
        assert!(s.get_issue("seen").unwrap().is_some());
    }

    #[test]
    fn org_change_clears_everything() {
        let s = store();
        assert!(!s.ensure_org("org1").unwrap());
        s.upsert_issues(&[IssueBuilder::new("a", "ENG-1", "a").build()], 100)
            .unwrap();
        assert!(!s.ensure_org("org1").unwrap());
        assert!(s.ensure_org("org2").unwrap());
        assert_eq!(s.get_issue("a").unwrap(), None);
        assert_eq!(s.meta_get("org_id").unwrap().as_deref(), Some("org2"));
    }

    #[test]
    fn branch_map_stores_hits_and_misses() {
        let s = store();
        s.branch_set("/repo", "me/eng-1-x", Some("ENG-1"), 10)
            .unwrap();
        s.branch_set("/repo", "main", None, 20).unwrap();
        assert_eq!(
            s.branch_get("/repo", "me/eng-1-x").unwrap(),
            Some((Some("ENG-1".into()), 10))
        );
        assert_eq!(s.branch_get("/repo", "main").unwrap(), Some((None, 20)));
        assert_eq!(s.branch_get("/other", "main").unwrap(), None);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test store`
Expected: 컴파일 실패 — `cannot find type `Store`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/store/mod.rs` 맨 위의 `mod cache;` 줄을 아래 구현으로 바꾼다(구현에 `mod cache;`가 들어 있다):

````rust
//! SQLite 캐시. 조회한 것을 저장해 다음에 즉시 보여준다.

mod cache;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config::ensure_private_dir;

/// 스키마 버전. 다르면 캐시를 비우고 새로 만든다.
pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE issues (
  id          TEXT PRIMARY KEY,
  identifier  TEXT NOT NULL UNIQUE,
  team_id     TEXT NOT NULL,
  number      INTEGER NOT NULL,
  title       TEXT NOT NULL,
  description TEXT,
  state_type  TEXT NOT NULL,
  priority    INTEGER NOT NULL,
  assignee_id TEXT,
  updated_at  TEXT NOT NULL,
  data        TEXT NOT NULL,
  fetched_at  INTEGER NOT NULL,
  viewed_at   INTEGER
);
CREATE TABLE comments (
  issue_id   TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE view_results (
  view_key   TEXT PRIMARY KEY,
  issue_ids  TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE team_refs (
  team_id    TEXT PRIMARY KEY,
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE workspace_labels (
  id         INTEGER PRIMARY KEY CHECK (id = 1),
  data       TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE TABLE branch_map (
  repo       TEXT NOT NULL,
  branch     TEXT NOT NULL,
  identifier TEXT,
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (repo, branch)
);
";

pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    /// 캐시 DB를 연다. 파일이 손상됐으면 지우고 새로 만든다.
    /// 잠금(BUSY) 같은 일시적 오류에는 파일을 지우지 않는다.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            ensure_private_dir(dir)?;
        }
        match Store::open_once(path) {
            Ok(store) => Ok(store),
            Err(e) if is_corrupt(&e) => {
                remove_db_files(path);
                Store::open_once(path).context("캐시 DB를 다시 만들지 못했어요")
            }
            Err(e) => Err(e.context("캐시 DB를 열지 못했어요")),
        }
    }

    pub fn open_in_memory() -> Result<Store> {
        let store = Store {
            conn: Connection::open_in_memory()?,
        };
        store.init()?;
        Ok(store)
    }

    fn open_once(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        set_private(path);
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let store = Store { conn };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        if version != 0 {
            self.drop_all_tables()?;
        }
        self.conn.execute_batch(SCHEMA)?;
        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    fn drop_all_tables(&self) -> Result<()> {
        let names: Vec<String> = {
            let mut st = self.conn.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )?;
            st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        for name in names {
            self.conn
                .execute_batch(&format!("DROP TABLE IF EXISTS \"{name}\""))?;
        }
        Ok(())
    }
}

fn is_corrupt(e: &anyhow::Error) -> bool {
    matches!(
        e.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(f, _))
            if matches!(f.code, rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
    )
}

/// DB 파일과 WAL 보조 파일을 지운다.
pub fn remove_db_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
}

fn set_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}
````

`src/store/cache.rs` 맨 위에 넣는다:

````rust
//! 캐시 읽기/쓰기.

use anyhow::Result;
use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::linear::types::{Comment, Issue};

impl Store {
    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// API 키의 워크스페이스가 바뀌었으면 캐시를 모두 비운다. 비웠으면 true.
    pub fn ensure_org(&self, org_id: &str) -> Result<bool> {
        let existing = self.meta_get("org_id")?;
        if existing.as_deref() == Some(org_id) {
            return Ok(false);
        }
        let cleared = existing.is_some();
        if cleared {
            self.clear_all()?;
        }
        self.meta_set("org_id", org_id)?;
        Ok(cleared)
    }

    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM meta; DELETE FROM issues; DELETE FROM comments; DELETE FROM view_results;
             DELETE FROM team_refs; DELETE FROM workspace_labels; DELETE FROM branch_map;",
        )?;
        Ok(())
    }

    /// 이슈를 저장한다. 보관·휴지통 이슈는 저장하지 않고 캐시에서 지운다.
    /// 지운 이슈 id를 돌려준다.
    pub fn upsert_issues(&self, issues: &[Issue], now_ms: i64) -> Result<Vec<String>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut removed = Vec::new();
        for issue in issues {
            if issue.is_gone() {
                tx.execute("DELETE FROM issues WHERE id = ?1", params![issue.id])?;
                tx.execute(
                    "DELETE FROM comments WHERE issue_id = ?1",
                    params![issue.id],
                )?;
                removed.push(issue.id.clone());
                continue;
            }
            // 팀 이동 등으로 식별자가 다른 이슈에 넘어간 경우 오래된 행을 지운다
            tx.execute(
                "DELETE FROM issues WHERE identifier = ?1 AND id <> ?2",
                params![issue.identifier, issue.id],
            )?;
            tx.execute(
                "INSERT INTO issues (id, identifier, team_id, number, title, description, state_type,
                                     priority, assignee_id, updated_at, data, fetched_at, viewed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL)
                 ON CONFLICT(id) DO UPDATE SET
                   identifier = excluded.identifier, team_id = excluded.team_id,
                   number = excluded.number, title = excluded.title,
                   description = excluded.description, state_type = excluded.state_type,
                   priority = excluded.priority, assignee_id = excluded.assignee_id,
                   updated_at = excluded.updated_at, data = excluded.data,
                   fetched_at = excluded.fetched_at",
                params![
                    issue.id,
                    issue.identifier,
                    issue.team.id,
                    issue.number,
                    issue.title,
                    issue.description,
                    issue.state.state_type,
                    issue.priority,
                    issue.assignee.as_ref().map(|a| a.id.as_str()),
                    issue.updated_at,
                    serde_json::to_string(issue)?,
                    now_ms,
                ],
            )?;
        }
        tx.commit()?;
        Ok(removed)
    }

    /// id 또는 식별자(대소문자 무시)로 찾는다.
    pub fn get_issue(&self, id_or_identifier: &str) -> Result<Option<Issue>> {
        let data: Option<String> = self
            .conn
            .query_row(
                "SELECT data FROM issues WHERE id = ?1 OR identifier = ?2",
                params![id_or_identifier, id_or_identifier.to_uppercase()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(match data {
            Some(d) => Some(serde_json::from_str(&d)?),
            None => None,
        })
    }

    pub fn remove_issue(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM issues WHERE id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM comments WHERE issue_id = ?1", params![id])?;
        Ok(())
    }

    /// 캐시된 모든 이슈 (로컬 검색용).
    pub fn all_issues(&self) -> Result<Vec<Issue>> {
        let mut st = self.conn.prepare("SELECT data FROM issues")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(issue) = serde_json::from_str(&row?) {
                out.push(issue);
            }
        }
        Ok(out)
    }

    /// 보기 탭 결과(순서 있는 id 목록)를 저장한다.
    pub fn set_view(&self, key: &str, ids: &[String], now_ms: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO view_results (view_key, issue_ids, fetched_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(view_key) DO UPDATE SET issue_ids = excluded.issue_ids, fetched_at = excluded.fetched_at",
            params![key, serde_json::to_string(ids)?, now_ms],
        )?;
        Ok(())
    }

    /// 저장된 보기 결과와 저장 시각. 캐시에 없는 id는 건너뛴다.
    pub fn get_view(&self, key: &str) -> Result<Option<(Vec<Issue>, i64)>> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT issue_ids, fetched_at FROM view_results WHERE view_key = ?1",
                params![key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((ids_json, fetched_at)) = row else {
            return Ok(None);
        };
        let ids: Vec<String> = serde_json::from_str(&ids_json)?;
        let mut issues = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(issue) = self.get_issue(&id)? {
                issues.push(issue);
            }
        }
        Ok(Some((issues, fetched_at)))
    }

    pub fn set_comments(&self, issue_id: &str, comments: &[Comment], now_ms: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO comments (issue_id, data, fetched_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(issue_id) DO UPDATE SET data = excluded.data, fetched_at = excluded.fetched_at",
            params![issue_id, serde_json::to_string(comments)?, now_ms],
        )?;
        Ok(())
    }

    pub fn get_comments(&self, issue_id: &str) -> Result<Option<(Vec<Comment>, i64)>> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT data, fetched_at FROM comments WHERE issue_id = ?1",
                params![issue_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((data, at)) => Some((serde_json::from_str(&data)?, at)),
            None => None,
        })
    }

    /// 상세 화면을 열었다고 기록한다 ("최근 본").
    pub fn mark_viewed(&self, issue_id: &str, now_ms: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE issues SET viewed_at = ?2 WHERE id = ?1",
            params![issue_id, now_ms],
        )?;
        Ok(())
    }

    /// 최근 본 이슈 (최근 순).
    pub fn recent_viewed(&self, limit: usize) -> Result<Vec<Issue>> {
        let mut st = self.conn.prepare(
            "SELECT data FROM issues WHERE viewed_at IS NOT NULL ORDER BY viewed_at DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit as i64], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(issue) = serde_json::from_str(&row?) {
                out.push(issue);
            }
        }
        Ok(out)
    }

    /// `cutoff_ms`보다 오래 안 받고 안 본 이슈와 그 코멘트를 지운다. 지운 이슈 수.
    pub fn evict_older_than(&self, cutoff_ms: i64) -> Result<usize> {
        let n = self.conn.execute(
            "DELETE FROM issues WHERE MAX(fetched_at, COALESCE(viewed_at, 0)) < ?1",
            params![cutoff_ms],
        )?;
        self.conn.execute(
            "DELETE FROM comments WHERE issue_id NOT IN (SELECT id FROM issues)",
            [],
        )?;
        Ok(n)
    }

    /// (레포, 브랜치) → 연결된 이슈 식별자. 저장된 "없음"은 `Some((None, at))`.
    pub fn branch_get(&self, repo: &str, branch: &str) -> Result<Option<(Option<String>, i64)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT identifier, fetched_at FROM branch_map WHERE repo = ?1 AND branch = ?2",
                params![repo, branch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn branch_set(
        &self,
        repo: &str,
        branch: &str,
        identifier: Option<&str>,
        now_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO branch_map (repo, branch, identifier, fetched_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(repo, branch) DO UPDATE SET identifier = excluded.identifier, fetched_at = excluded.fetched_at",
            params![repo, branch, identifier, now_ms],
        )?;
        Ok(())
    }
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test store`
Expected: `test result: ok. 16 passed`(mod.rs 6개 + cache.rs 10개)

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/store/mod.rs src/store/cache.rs
git commit -m "feat(store): SQLite 캐시 저장소" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 8: 로컬 검색 순위와 결과 합치기

스펙 4.5장 순위는 식별자·번호 일치 → 제목 퍼지(nucleo) → 본문 포함 순이다. 점수가 같으면 상태 순서, 그다음 최근 수정 순으로 정렬한다.
- 서버 결과는 로컬 텍스트 매칭에 실패해도 남긴다. 코멘트에서만 일치한 경우가 있기 때문이다.
- `sort_mine`은 스펙 4.4장의 "내 이슈" 정렬이다.
- 성능 목표는 캐시 5천 건 기준 키 입력당 16ms다. release 모드에서 따로 확인한다.

**Files:**
- Create: `src/search/rank.rs`
- Modify: `src/search/mod.rs`
- Test: `src/search/rank.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: `query::{ParsedQuery, StateMatch, AssigneeMatch, parse}`(Task 5), `types::Issue`(Task 2), `IssueBuilder`(테스트)
- Produces:
  - `rank::state_order(&str) -> u8`(started 0 → unstarted 1 → backlog 2 → triage 3 → completed 4 → canceled 5 → duplicate 6)
  - `rank::priority_order(i64) -> i64`(긴급 먼저, 없음은 마지막)
  - `rank::sort_mine(&mut [Issue])`
  - `rank::SearchIndex::new(Vec<Issue>) -> SearchIndex`, `.search(&ParsedQuery, viewer_id: Option<&str>) -> Vec<Issue>`, `.len()`, `.is_empty()`
  - `rank::merge(local: &[Issue], server: &[Issue], &ParsedQuery, viewer_id: Option<&str>) -> Vec<Issue>`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/search/mod.rs`:

````rust
//! 검색어 해석과 로컬 순위.

pub mod query;
pub mod rank;
````

`src/search/rank.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::query::parse;
    use crate::test_support::IssueBuilder;

    fn ids(issues: &[Issue]) -> Vec<&str> {
        issues.iter().map(|i| i.identifier.as_str()).collect()
    }

    fn index() -> SearchIndex {
        SearchIndex::new(vec![
            IssueBuilder::new("a", "ENG-131", "로그인 버튼 비활성 버그")
                .state("In Progress", "started")
                .labels(&["bug", "frontend"])
                .assignee("me", "김민수")
                .priority(2)
                .build(),
            IssueBuilder::new("b", "ENG-140", "결제 페이지 리팩터링")
                .description("로그인 이후 결제 흐름 정리")
                .updated("2026-10-05T00:00:00.000Z")
                .build(),
            IssueBuilder::new("c", "OPS-21", "로그인 서버 모니터링")
                .state("Done", "completed")
                .assignee("u2", "이영희")
                .build(),
            IssueBuilder::new("d", "ENG-7", "다크 모드 색상 정리").build(),
        ])
    }

    #[test]
    fn identifier_and_number_come_first() {
        let r = index().search(&parse("eng-7"), None);
        assert_eq!(ids(&r)[0], "ENG-7");
        let r = index().search(&parse("131"), None);
        assert_eq!(ids(&r)[0], "ENG-131");
    }

    #[test]
    fn title_fuzzy_beats_body_and_body_matches_are_included() {
        let r = index().search(&parse("로그인"), None);
        // 제목 일치 (진행 중이 완료보다 위) → 본문 일치
        assert_eq!(ids(&r), vec!["ENG-131", "OPS-21", "ENG-140"]);
    }

    #[test]
    fn filters_apply_without_text() {
        let r = index().search(&parse("l:bug"), None);
        assert_eq!(ids(&r), vec!["ENG-131"]);
        let r = index().search(&parse("s:완료"), None);
        assert_eq!(ids(&r), vec!["OPS-21"]);
        let r = index().search(&parse("s:progress #eng"), None);
        assert_eq!(ids(&r), vec!["ENG-131"]);
        let r = index().search(&parse("p:high"), None);
        assert_eq!(ids(&r), vec!["ENG-131"]);
        let r = index().search(&parse("@영희"), None);
        assert_eq!(ids(&r), vec!["OPS-21"]);
    }

    #[test]
    fn me_needs_viewer_id() {
        assert!(index().search(&parse("@나"), None).is_empty());
        assert_eq!(
            ids(&index().search(&parse("@나"), Some("me"))),
            vec!["ENG-131"]
        );
    }

    #[test]
    fn empty_query_sorts_by_state_then_recent() {
        let r = index().search(&parse(""), None);
        assert_eq!(ids(&r), vec!["ENG-131", "ENG-140", "ENG-7", "OPS-21"]);
    }

    #[test]
    fn decomposed_titles_still_match() {
        let nfd: String = "로그인 문제".nfd().collect();
        let idx = SearchIndex::new(vec![IssueBuilder::new("x", "ENG-9", &nfd).build()]);
        assert_eq!(ids(&idx.search(&parse("로그인"), None)), vec!["ENG-9"]);
    }

    #[test]
    fn no_match_returns_nothing() {
        assert!(index().search(&parse("존재하지않는단어"), None).is_empty());
    }

    #[test]
    fn merge_prefers_server_copy_and_keeps_server_only_hits() {
        let local = vec![IssueBuilder::new("a", "ENG-131", "로그인 버튼").build()];
        let server = vec![
            IssueBuilder::new("a", "ENG-131", "로그인 버튼 (수정됨)").build(),
            // 코멘트에서만 일치한 결과: 로컬 텍스트 매칭은 실패하지만 남긴다
            IssueBuilder::new("z", "ENG-500", "알림 설정").build(),
        ];
        let r = merge(&local, &server, &parse("로그인"), None);
        assert_eq!(ids(&r), vec!["ENG-131", "ENG-500"]);
        assert_eq!(r[0].title, "로그인 버튼 (수정됨)");
    }

    #[test]
    fn sort_mine_orders_state_priority_recent() {
        let mut issues = vec![
            IssueBuilder::new("1", "ENG-1", "a").priority(0).build(),
            IssueBuilder::new("2", "ENG-2", "b")
                .state("Doing", "started")
                .priority(3)
                .build(),
            IssueBuilder::new("3", "ENG-3", "c").priority(1).build(),
            IssueBuilder::new("4", "ENG-4", "d")
                .state("Backlog", "backlog")
                .build(),
        ];
        sort_mine(&mut issues);
        assert_eq!(ids(&issues), vec!["ENG-2", "ENG-3", "ENG-1", "ENG-4"]);
    }

    /// 성능: 캐시 5천 건 기준 키 입력당 16ms 이내. `cargo test --release -- --ignored`로 실행한다.
    #[test]
    #[ignore]
    fn search_5000_under_16ms() {
        let issues: Vec<Issue> = (0..5000)
            .map(|i| {
                IssueBuilder::new(
                    &format!("id{i}"),
                    &format!("ENG-{i}"),
                    &format!("로그인 기능 {i} 개선 작업"),
                )
                .description("세션 만료 시 다시 로그인하도록 처리하고 결제 흐름을 정리한다")
                .build()
            })
            .collect();
        let idx = SearchIndex::new(issues);
        let q = parse("로그인 개선");
        let start = std::time::Instant::now();
        let r = idx.search(&q, None);
        let elapsed = start.elapsed();
        assert_eq!(r.len(), 5000);
        assert!(elapsed.as_millis() < 16, "{elapsed:?}");
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test search::rank`
Expected: 컴파일 실패 — `cannot find type `SearchIndex`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/search/rank.rs` 맨 위에 넣는다:

````rust
//! 로컬 검색: 캐시된 이슈를 즉시 거르고 순위를 매긴다.

use std::cmp::Ordering;
use std::collections::HashSet;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use unicode_normalization::UnicodeNormalization;

use super::query::{AssigneeMatch, ParsedQuery, StateMatch};
use crate::linear::types::Issue;

/// 상태 타입 정렬 순서. 작을수록 위.
pub fn state_order(state_type: &str) -> u8 {
    match state_type {
        "started" => 0,
        "unstarted" => 1,
        "backlog" => 2,
        "triage" => 3,
        "completed" => 4,
        "canceled" => 5,
        "duplicate" => 6,
        _ => 7,
    }
}

/// 우선순위 정렬 순서: 긴급(1) → 낮음(4) → 없음(0).
pub fn priority_order(priority: i64) -> i64 {
    if priority == 0 { 5 } else { priority }
}

/// "내 이슈" 정렬: 상태 → 우선순위 → 최근 수정.
pub fn sort_mine(issues: &mut [Issue]) {
    issues.sort_by(|a, b| {
        state_order(&a.state.state_type)
            .cmp(&state_order(&b.state.state_type))
            .then_with(|| priority_order(a.priority).cmp(&priority_order(b.priority)))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });
}

fn norm(s: &str) -> String {
    s.nfc().collect::<String>().to_lowercase()
}

struct Entry {
    issue: Issue,
    /// "식별자 제목" (소문자, NFC)
    hay: String,
    /// 본문 (소문자, NFC)
    desc: String,
}

impl Entry {
    fn new(issue: Issue) -> Entry {
        let hay = format!("{} {}", issue.identifier.to_lowercase(), norm(&issue.title));
        let desc = norm(issue.description.as_deref().unwrap_or(""));
        Entry { issue, hay, desc }
    }
}

/// 캐시된 이슈의 검색 색인. 문자열 정규화를 미리 해 둔다.
pub struct SearchIndex {
    entries: Vec<Entry>,
}

impl SearchIndex {
    pub fn new(issues: Vec<Issue>) -> SearchIndex {
        SearchIndex {
            entries: issues.into_iter().map(Entry::new).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 검색어로 거르고 순위대로 돌려준다.
    pub fn search(&self, q: &ParsedQuery, viewer_id: Option<&str>) -> Vec<Issue> {
        rank(&self.entries, &HashSet::new(), q, viewer_id)
    }
}

/// 로컬 결과와 서버 결과를 합친다. 같은 id면 서버 쪽(최신)을 쓴다.
/// 서버 결과는 서버가 조건을 확인했으므로 로컬 텍스트 매칭에 실패해도 남긴다.
pub fn merge(
    local: &[Issue],
    server: &[Issue],
    q: &ParsedQuery,
    viewer_id: Option<&str>,
) -> Vec<Issue> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for issue in server.iter().chain(local) {
        if seen.insert(issue.id.clone()) {
            entries.push(Entry::new(issue.clone()));
        }
    }
    let keep: HashSet<String> = server.iter().map(|i| i.id.clone()).collect();
    rank(&entries, &keep, q, viewer_id)
}

#[derive(Debug, Clone, Copy)]
struct Rank {
    /// 0 식별자·번호 일치, 1 제목 퍼지, 2 본문 포함, 3 텍스트 조건 없음
    tier: u8,
    score: u32,
}

fn rank(
    entries: &[Entry],
    keep: &HashSet<String>,
    q: &ParsedQuery,
    viewer_id: Option<&str>,
) -> Vec<Issue> {
    let terms = text_terms(q);
    let pattern = (!terms.is_empty())
        .then(|| Pattern::parse(&terms.join(" "), CaseMatching::Ignore, Normalization::Smart));
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut scored: Vec<(Rank, &Issue)> = Vec::new();
    for e in entries {
        let kept = keep.contains(&e.issue.id);
        if !kept && !passes_filters(&e.issue, q, viewer_id) {
            continue;
        }
        let rank = if !q.has_text() {
            Rank { tier: 3, score: 0 }
        } else if exact_match(&e.issue, q) {
            Rank { tier: 0, score: 0 }
        } else if let Some(score) = pattern
            .as_ref()
            .and_then(|p| p.score(Utf32Str::new(&e.hay, &mut buf), &mut matcher))
        {
            Rank { tier: 1, score }
        } else if terms.iter().all(|t| e.desc.contains(t.as_str())) || kept {
            Rank { tier: 2, score: 0 }
        } else {
            continue;
        };
        scored.push((rank, &e.issue));
    }
    scored.sort_by(|a, b| compare(a.0, a.1, b.0, b.1));
    scored.into_iter().map(|(_, issue)| issue.clone()).collect()
}

fn text_terms(q: &ParsedQuery) -> Vec<String> {
    let mut terms: Vec<String> = q.words.iter().map(|w| norm(w)).collect();
    if let Some((key, n)) = &q.identifier {
        terms.push(format!("{}-{n}", key.to_lowercase()));
    }
    if let Some(n) = q.number {
        terms.push(n.to_string());
    }
    terms
}

fn exact_match(issue: &Issue, q: &ParsedQuery) -> bool {
    if let Some((key, n)) = &q.identifier
        && issue.team.key.eq_ignore_ascii_case(key)
        && issue.number == *n
    {
        return true;
    }
    q.number == Some(issue.number)
}

fn passes_filters(issue: &Issue, q: &ParsedQuery, viewer_id: Option<&str>) -> bool {
    if let Some(state) = &q.state {
        let ok = match state {
            StateMatch::Type(t) => issue.state.state_type == *t,
            StateMatch::Name(n) => norm(&issue.state.name).contains(&norm(n)),
        };
        if !ok {
            return false;
        }
    }
    if let Some(label) = &q.label {
        let label = norm(label);
        if !issue
            .labels
            .nodes
            .iter()
            .any(|l| norm(&l.name).contains(&label))
        {
            return false;
        }
    }
    match &q.assignee {
        Some(AssigneeMatch::Me) => {
            let mine =
                viewer_id.is_some() && issue.assignee.as_ref().map(|a| a.id.as_str()) == viewer_id;
            if !mine {
                return false;
            }
        }
        Some(AssigneeMatch::Name(name)) => {
            let name = norm(name);
            let ok = issue.assignee.as_ref().is_some_and(|a| {
                norm(&a.name).contains(&name) || norm(&a.display_name).contains(&name)
            });
            if !ok {
                return false;
            }
        }
        None => {}
    }
    if let Some(key) = &q.team
        && !issue.team.key.eq_ignore_ascii_case(key)
    {
        return false;
    }
    if let Some(p) = q.priority
        && issue.priority != p
    {
        return false;
    }
    true
}

fn compare(ra: Rank, a: &Issue, rb: Rank, b: &Issue) -> Ordering {
    ra.tier
        .cmp(&rb.tier)
        .then_with(|| rb.score.cmp(&ra.score))
        .then_with(|| state_order(&a.state.state_type).cmp(&state_order(&b.state.state_type)))
        .then_with(|| b.updated_at.cmp(&a.updated_at))
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test search::rank`
Expected: `test result: ok. 9 passed; 0 failed; 1 ignored`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

성능 테스트(release):

````bash
cargo test --release search_5000_under_16ms -- --ignored
````

Expected: `test result: ok. 1 passed`

- [ ] **Step 5: 커밋**

````bash
git add src/search/mod.rs src/search/rank.rs
git commit -m "feat(search): 로컬 순위와 서버 결과 합치기" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 9: markdown 렌더러

스펙 5장이다. 주요 동작은 다음과 같다.
- pulldown-cmark 이벤트를 받아 ratatui `Line`으로 그린다. 표, 취소선, 체크박스 옵션을 켠다.
- 폭은 표시 폭 기준이다. 보통은 마지막 공백에서 줄을 바꾸고, 공백이 없으면 글자 단위로 바꾼다. 코드 블록은 글자 단위로만 바꾼다.
- 링크와 이미지에는 등장 순서대로 번호를 매긴다.
- 그리기 전에 **NFC 정규화와 제어 문자 제거**를 한다(Review Focus).
- CLI 출력용 `to_plain`과 `to_ansi`도 여기서 만든다.

**Files:**
- Create: `src/markdown.rs`
- Modify: `src/lib.rs`
- Test: `src/markdown.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: 없음
- Produces:
  - `markdown::LinkKind::{Link, Image}`, `markdown::LinkTarget { index: usize, kind: LinkKind, url: String, label: String }`
  - `markdown::Rendered { lines: Vec<Line<'static>>, links: Vec<LinkTarget> }`
  - `markdown::Theme { heading, rule, code, quote, link, dim, identifier }`(모두 `Style`, `Default` 구현)
  - `markdown::sanitize(&str) -> String`(제어 문자 제거, `\n`·`\t` 유지)
  - `markdown::render(md: &str, width: u16, &Theme) -> Rendered`(폭 최소 10)
  - `markdown::to_plain(&[Line]) -> String`, `markdown::to_ansi(&[Line]) -> String`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/lib.rs` 전체:

````rust
//! herdr-linear: herdr 안에서 Linear를 빠르게 조회한다.

pub mod config;
pub mod linear;
pub mod markdown;
pub mod search;
pub mod store;

#[cfg(test)]
pub mod test_support;
````

`src/markdown.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;

    fn plain(md: &str, width: u16) -> Vec<String> {
        let r = render(md, width, &Theme::default());
        to_plain(&r.lines).split('\n').map(str::to_string).collect()
    }

    #[test]
    fn heading_rule_and_paragraph() {
        assert_eq!(
            plain("# 제목\n\n본문 문단입니다.", 20),
            vec![
                "제목".to_string(),
                "─".repeat(20),
                String::new(),
                "본문 문단입니다.".to_string()
            ]
        );
    }

    #[test]
    fn small_headings_have_no_rule() {
        assert_eq!(
            plain("### 작은 제목\n#### 더 작은", 20),
            vec!["작은 제목", "", "더 작은"]
        );
    }

    #[test]
    fn heading_styles() {
        let theme = Theme::default();
        let r = render("## 둘\n\n#### 넷", 20, &theme);
        assert_eq!(r.lines[0].spans[0].style, theme.heading);
        let h4 = r
            .lines
            .iter()
            .find(|l| to_plain(std::slice::from_ref(l)) == "넷")
            .unwrap();
        assert_eq!(h4.spans[0].style, Style::new().add_modifier(Modifier::BOLD));
    }

    #[test]
    fn paragraphs_are_separated_and_breaks_work() {
        assert_eq!(
            plain("첫 문단\n\n둘째 문단", 20),
            vec!["첫 문단", "", "둘째 문단"]
        );
        assert_eq!(plain("줄1\n줄2", 20), vec!["줄1 줄2"]);
        assert_eq!(plain("줄1  \n줄2", 20), vec!["줄1", "줄2"]);
    }

    #[test]
    fn nested_bullets_and_numbers() {
        assert_eq!(
            plain("- 하나\n- 둘\n  - 셋\n", 20),
            vec!["• 하나", "• 둘", "  • 셋"]
        );
        assert_eq!(plain("1. 첫째\n2. 둘째\n", 20), vec!["1. 첫째", "2. 둘째"]);
    }

    #[test]
    fn task_list_boxes() {
        assert_eq!(
            plain("- [x] 끝\n- [ ] 할 일\n", 20),
            vec!["☑ 끝", "☐ 할 일"]
        );
    }

    #[test]
    fn list_item_wraps_under_its_text() {
        assert_eq!(
            plain("- 가나다 라마바 사아자", 12),
            vec!["• 가나다", "  라마바", "  사아자"]
        );
    }

    #[test]
    fn code_block_keeps_indent_and_hard_wraps() {
        assert_eq!(
            plain("```rust\nfn main() {}\n```", 20),
            vec!["  fn main() {}"]
        );
        assert_eq!(
            plain("```\n0123456789012345678901234\n```", 20),
            vec!["  012345678901234567", "  8901234"]
        );
    }

    #[test]
    fn code_block_uses_code_style() {
        let theme = Theme::default();
        let r = render("```\nx\n```", 20, &theme);
        assert!(r.lines[0].spans.iter().all(|s| s.style == theme.code));
    }

    #[test]
    fn quote_has_bar_and_dim_text() {
        let r = render("> 인용문", 20, &Theme::default());
        assert_eq!(to_plain(&r.lines), "▌ 인용문");
        assert!(
            r.lines[0].spans[1]
                .style
                .add_modifier
                .contains(Modifier::DIM)
        );
    }

    #[test]
    fn table_aligns_korean_width() {
        assert_eq!(
            plain("| 이름 | 값 |\n|---|---|\n| 로그인 | 1 |\n", 40),
            vec![
                "┌────────┬────┐",
                "│ 이름   │ 값 │",
                "├────────┼────┤",
                "│ 로그인 │ 1  │",
                "└────────┴────┘",
            ]
        );
    }

    #[test]
    fn wide_table_cells_are_truncated() {
        assert_eq!(
            plain("| 설명 |\n|---|\n| 아주 긴 설명 텍스트입니다 |\n", 12),
            vec![
                "┌──────────┐",
                "│ 설명     │",
                "├──────────┤",
                "│ 아주 긴… │",
                "└──────────┘"
            ]
        );
    }

    #[test]
    fn links_and_images_are_numbered() {
        let r = render(
            "자세한 건 [문서](https://x.dev/doc)와 ![스크린샷](https://x.dev/a/shot.png) 참고, ![](https://x.dev/b/c.png?v=1)",
            80,
            &Theme::default(),
        );
        assert_eq!(
            to_plain(&r.lines),
            "자세한 건 문서 [1]와 [이미지 2: 스크린샷] 참고, [이미지 3: c.png]"
        );
        assert_eq!(
            r.links,
            vec![
                LinkTarget {
                    index: 1,
                    kind: LinkKind::Link,
                    url: "https://x.dev/doc".into(),
                    label: "문서".into()
                },
                LinkTarget {
                    index: 2,
                    kind: LinkKind::Image,
                    url: "https://x.dev/a/shot.png".into(),
                    label: "스크린샷".into()
                },
                LinkTarget {
                    index: 3,
                    kind: LinkKind::Image,
                    url: "https://x.dev/b/c.png?v=1".into(),
                    label: "c.png".into()
                },
            ]
        );
    }

    #[test]
    fn identifiers_are_highlighted() {
        let theme = Theme::default();
        let r = render("ENG-123을 보세요. abcENG-12는 아님", 80, &theme);
        let hits: Vec<&str> = r.lines[0]
            .spans
            .iter()
            .filter(|s| s.style == theme.identifier)
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(hits, vec!["ENG-123"]);
    }

    #[test]
    fn korean_wraps_at_spaces_then_chars() {
        assert_eq!(
            plain("가나다 라마바 사아자", 10),
            vec!["가나다", "라마바", "사아자"]
        );
        assert_eq!(plain("가나다라마바사", 10), vec!["가나다라마", "바사"]);
    }

    #[test]
    fn html_is_shown_as_text() {
        assert_eq!(
            plain("<details>숨김</details>", 40),
            vec!["<details>숨김</details>"]
        );
    }

    #[test]
    fn rule_and_inline_code() {
        assert_eq!(
            plain("위\n\n---\n\n`코드` 아래", 10),
            vec![
                "위".to_string(),
                String::new(),
                "─".repeat(10),
                String::new(),
                "코드 아래".to_string()
            ]
        );
    }

    #[test]
    fn ansi_output_has_escape_codes() {
        let r = render("**굵게**", 20, &Theme::default());
        let ansi = to_ansi(&r.lines);
        assert!(ansi.contains("\u{1b}["), "{ansi:?}");
        assert!(ansi.contains("굵게"));
    }

    #[test]
    fn empty_input_is_empty() {
        let r = render("", 20, &Theme::default());
        assert!(r.lines.is_empty());
    }

    #[test]
    fn control_characters_are_removed() {
        // ESC 시퀀스(색 바꾸기, OSC 52 클립보드 쓰기)가 터미널에 그대로 가면 안 된다
        assert_eq!(
            plain("a\u{1b}[31mb \u{1b}]52;c;ZXZpbA==\u{7}c", 40),
            vec!["a[31mb ]52;c;ZXZpbA==c"]
        );
        assert_eq!(sanitize("줄1\n\t줄2\u{0}"), "줄1\n\t줄2");
    }

    #[test]
    fn decomposed_hangul_is_normalized_before_wrapping() {
        let nfd: String = "가나다 라마바".nfd().collect();
        assert_eq!(plain(&nfd, 10), vec!["가나다", "라마바"]);
    }

    #[test]
    fn tiny_width_and_wide_tables_do_not_panic() {
        let r = render("# 제목\n\n본문 텍스트입니다", 3, &Theme::default());
        assert!(
            r.lines.iter().all(|l| l.width() <= 10),
            "{:?}",
            to_plain(&r.lines)
        );
        let table = "| a | b | c | d | e |\n|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 |\n";
        let r = render(table, 10, &Theme::default());
        assert_eq!(r.lines.len(), 5);
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test markdown`
Expected: 컴파일 실패 — `cannot find function `render`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/markdown.rs` 맨 위에 넣는다:

````rust
//! Linear markdown → ratatui 줄. 폭은 표시 폭(한글 2칸) 기준.

use std::sync::LazyLock;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Link,
    Image,
}

/// 본문에 나온 링크·이미지. `index`는 1부터, 등장 순서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget {
    pub index: usize,
    pub kind: LinkKind,
    pub url: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    pub links: Vec<LinkTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub heading: Style,
    pub rule: Style,
    pub code: Style,
    pub quote: Style,
    pub link: Style,
    pub dim: Style,
    pub identifier: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            heading: Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            rule: Style::new().fg(Color::DarkGray),
            code: Style::new().fg(Color::Yellow).bg(Color::Rgb(40, 40, 40)),
            quote: Style::new().fg(Color::DarkGray),
            link: Style::new()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            dim: Style::new().fg(Color::DarkGray),
            identifier: Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        }
    }
}

/// 터미널 제어 문자(ESC 등)를 지운다. 줄바꿈과 탭은 남긴다.
/// Linear 내용은 신뢰할 수 없는 입력이라 터미널에 그대로 쓰면 안 된다.
pub fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// markdown을 `width` 칸에 맞춰 그린다. 폭은 최소 10칸으로 본다.
/// 그리기 전에 NFC로 정규화하고 제어 문자를 지운다.
pub fn render(md: &str, width: u16, theme: &Theme) -> Rendered {
    let md = sanitize(&md.nfc().collect::<String>());
    let mut r = Renderer::new(usize::from(width).max(10), *theme);
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for ev in Parser::new_ext(&md, opts) {
        r.event(ev);
    }
    r.finish()
}

/// 스타일 없이 텍스트만 (테스트·파이프 출력용).
pub fn to_plain(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// ANSI 이스케이프 문자열 (터미널 출력용).
pub fn to_ansi(lines: &[Line<'_>]) -> String {
    use ratatui::backend::IntoCrossterm;
    use ratatui::crossterm::style::{ContentStyle, StyledContent};
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            let cs: ContentStyle = span.style.into_crossterm();
            out.push_str(&StyledContent::new(cs, span.content.as_ref()).to_string());
        }
    }
    out
}

/// 대문자 팀 키 + 번호 (예: ENG-123). 앞 글자가 영숫자면 제외한다.
static IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z][A-Z0-9]{0,9}-[0-9]+").expect("valid regex"));

fn find_identifiers(s: &str) -> Vec<(usize, usize)> {
    IDENTIFIER
        .find_iter(s)
        .filter(|m| {
            s[..m.start()]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-')
        })
        .map(|m| (m.start(), m.end()))
        .collect()
}

type Seg = (String, Style);

#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: Option<String>,
    header_rows: usize,
}

struct Renderer {
    width: usize,
    theme: Theme,
    out: Vec<Line<'static>>,
    links: Vec<LinkTarget>,
    inline: Vec<Seg>,
    styles: Vec<Style>,
    /// 목록마다 다음 번호. `None`이면 글머리표 목록.
    lists: Vec<Option<u64>>,
    quote: usize,
    marker: Option<String>,
    code: Option<String>,
    link: Option<(String, usize)>,
    image: Option<(String, String)>,
    table: Option<Table>,
    need_blank: bool,
}

impl Renderer {
    fn new(width: usize, theme: Theme) -> Renderer {
        Renderer {
            width,
            theme,
            out: Vec::new(),
            links: Vec::new(),
            inline: Vec::new(),
            styles: Vec::new(),
            lists: Vec::new(),
            quote: 0,
            marker: None,
            code: None,
            link: None,
            image: None,
            table: None,
            need_blank: false,
        }
    }

    fn finish(mut self) -> Rendered {
        self.flush();
        while self.out.last().is_some_and(|l| l.spans.is_empty()) {
            self.out.pop();
        }
        Rendered {
            lines: self.out,
            links: self.links,
        }
    }

    fn cur_style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, style: Style) {
        let s = self.cur_style().patch(style);
        self.styles.push(s);
    }

    fn push_seg(&mut self, text: String, style: Style) {
        if !text.is_empty() {
            self.inline.push((text, style));
        }
    }

    fn push_text(&mut self, s: &str) {
        let style = self.cur_style();
        if self.link.is_some() {
            self.push_seg(s.to_string(), style);
            return;
        }
        let mut last = 0;
        for (start, end) in find_identifiers(s) {
            self.push_seg(s[last..start].to_string(), style);
            self.push_seg(
                s[start..end].to_string(),
                style.patch(self.theme.identifier),
            );
            last = end;
        }
        self.push_seg(s[last..].to_string(), style);
    }

    fn blank_if_needed(&mut self) {
        if self.need_blank && !self.out.is_empty() {
            self.out.push(Line::default());
        }
        self.need_blank = false;
    }

    fn event(&mut self, ev: Event<'_>) {
        if let Some(t) = self.table.as_mut() {
            match ev {
                Event::Start(Tag::TableCell) => t.cell = Some(String::new()),
                Event::End(TagEnd::TableCell) => {
                    let c = t.cell.take().unwrap_or_default();
                    t.row.push(c.trim().to_string());
                }
                Event::End(TagEnd::TableHead) => {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                    t.header_rows = 1;
                }
                Event::End(TagEnd::TableRow) => {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
                Event::Text(s) | Event::Code(s) | Event::Html(s) | Event::InlineHtml(s) => {
                    if let Some(c) = t.cell.as_mut() {
                        c.push_str(&s);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if let Some(c) = t.cell.as_mut() {
                        c.push(' ');
                    }
                }
                Event::End(TagEnd::Table) => {
                    let t = self.table.take().unwrap_or_default();
                    self.render_table(t);
                }
                _ => {}
            }
            return;
        }
        if let Some(code) = self.code.as_mut() {
            match ev {
                Event::Text(s) => code.push_str(&s),
                Event::End(TagEnd::CodeBlock) => {
                    let c = self.code.take().unwrap_or_default();
                    self.render_code(&c);
                }
                _ => {}
            }
            return;
        }
        if let Some((_, alt)) = self.image.as_mut() {
            match ev {
                Event::Text(s) | Event::Code(s) => alt.push_str(&s),
                Event::End(TagEnd::Image) => {
                    if let Some((url, alt)) = self.image.take() {
                        self.push_image(url, alt);
                    }
                }
                _ => {}
            }
            return;
        }
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(s) => self.push_text(&s),
            Event::Code(s) => {
                let st = self.cur_style().patch(self.theme.code);
                self.push_seg(s.to_string(), st);
            }
            Event::Html(s) | Event::InlineHtml(s) => {
                let st = self.theme.dim;
                self.push_seg(s.trim_end_matches('\n').to_string(), st);
            }
            Event::SoftBreak => {
                let st = self.cur_style();
                self.push_seg(" ".to_string(), st);
            }
            Event::HardBreak => {
                let st = self.cur_style();
                self.push_seg("\n".to_string(), st);
            }
            Event::Rule => {
                self.flush();
                self.blank_if_needed();
                self.out.push(Line::from(Span::styled(
                    "─".repeat(self.width),
                    self.theme.rule,
                )));
                self.need_blank = true;
            }
            Event::TaskListMarker(done) => {
                self.marker = Some(if done { "☑ " } else { "☐ " }.to_string());
            }
            Event::FootnoteReference(s) => {
                let st = self.theme.dim;
                self.push_seg(format!("[^{s}]"), st);
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush();
                let style = match level {
                    HeadingLevel::H1 | HeadingLevel::H2 | HeadingLevel::H3 => self.theme.heading,
                    _ => Style::new().add_modifier(Modifier::BOLD),
                };
                self.push_style(style);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = Some(String::new());
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush();
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "• ".to_string(),
                };
                self.marker = Some(marker);
            }
            Tag::Emphasis => self.push_style(Style::new().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.push_style(Style::new().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self.push_style(Style::new().add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { dest_url, .. } => {
                self.link = Some((dest_url.to_string(), self.inline.len()));
                self.push_style(self.theme.link);
            }
            Tag::Image { dest_url, .. } => {
                self.image = Some((dest_url.to_string(), String::new()));
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(Table::default());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock => {
                self.flush();
                self.need_blank = true;
            }
            TagEnd::Heading(level) => {
                self.flush();
                self.styles.pop();
                if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    self.out.push(Line::from(Span::styled(
                        "─".repeat(self.width),
                        self.theme.rule,
                    )));
                }
                self.need_blank = true;
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
                self.need_blank = true;
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
                if self.lists.is_empty() {
                    self.need_blank = true;
                }
            }
            TagEnd::Item => self.flush(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link => {
                self.styles.pop();
                if let Some((url, start)) = self.link.take() {
                    let label: String = self.inline[start..]
                        .iter()
                        .map(|(s, _)| s.as_str())
                        .collect();
                    let index = self.links.len() + 1;
                    self.links.push(LinkTarget {
                        index,
                        kind: LinkKind::Link,
                        url,
                        label: label.trim().to_string(),
                    });
                    let st = self.theme.dim;
                    self.push_seg(format!(" [{index}]"), st);
                }
            }
            _ => {}
        }
    }

    fn push_image(&mut self, url: String, alt: String) {
        let index = self.links.len() + 1;
        let label = if alt.trim().is_empty() {
            file_name(&url)
        } else {
            alt.trim().to_string()
        };
        self.links.push(LinkTarget {
            index,
            kind: LinkKind::Image,
            url,
            label: label.clone(),
        });
        let st = self.theme.dim;
        self.push_seg(format!("[이미지 {index}: {label}]"), st);
    }

    /// 목록·인용 접두사. (첫 줄, 이어지는 줄)
    fn prefixes(&mut self) -> (Vec<Seg>, Vec<Seg>) {
        let mut base: Vec<Seg> = Vec::new();
        for _ in 0..self.quote {
            base.push(("▌ ".to_string(), self.theme.quote));
        }
        let depth = self.lists.len();
        if depth > 1 {
            base.push(("  ".repeat(depth - 1), Style::default()));
        }
        match self.marker.take() {
            Some(m) => {
                let w = m.width();
                let mut first = base.clone();
                first.push((m, Style::default()));
                let mut rest = base;
                rest.push((" ".repeat(w), Style::default()));
                (first, rest)
            }
            None => {
                if depth > 0 {
                    base.push(("  ".to_string(), Style::default()));
                }
                (base.clone(), base)
            }
        }
    }

    fn flush(&mut self) {
        if self.inline.is_empty() && self.marker.is_none() {
            return;
        }
        let mut segs = std::mem::take(&mut self.inline);
        if self.quote > 0 {
            for (_, st) in segs.iter_mut() {
                *st = st.add_modifier(Modifier::DIM);
            }
        }
        self.blank_if_needed();
        let (first, rest) = self.prefixes();
        let lines = wrap(&segs, &first, &rest, self.width, false);
        self.out.extend(lines);
    }

    fn render_code(&mut self, code: &str) {
        self.blank_if_needed();
        let (mut first, mut rest) = self.prefixes();
        first.push(("  ".to_string(), self.theme.code));
        rest.push(("  ".to_string(), self.theme.code));
        for raw in code.trim_end_matches('\n').split('\n') {
            let text = raw.replace('\t', "    ");
            let segs = vec![(text, self.theme.code)];
            self.out
                .extend(wrap(&segs, &first, &rest, self.width, true));
        }
        self.need_blank = true;
    }

    fn render_table(&mut self, t: Table) {
        let cols = t.rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        self.blank_if_needed();
        let mut widths = vec![1usize; cols];
        for row in &t.rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.width());
            }
        }
        let total = |w: &[usize]| 1 + w.iter().map(|x| x + 3).sum::<usize>();
        while total(&widths) > self.width {
            let (i, max) = widths
                .iter()
                .copied()
                .enumerate()
                .max_by_key(|(_, w)| *w)
                .unwrap_or((0, 0));
            if max <= 3 {
                break;
            }
            widths[i] -= 1;
        }
        let rule = self.theme.rule;
        self.out.push(border(&widths, "┌", "┬", "┐", rule));
        for (ri, row) in t.rows.iter().enumerate() {
            let style = if ri < t.header_rows {
                Style::new().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let mut spans = vec![Span::styled("│", rule)];
            for (i, w) in widths.iter().enumerate() {
                let cell = row.get(i).map(String::as_str).unwrap_or("");
                spans.push(Span::styled(format!(" {} ", fit(cell, *w)), style));
                spans.push(Span::styled("│", rule));
            }
            self.out.push(Line::from(spans));
            if ri + 1 == t.header_rows && t.rows.len() > t.header_rows {
                self.out.push(border(&widths, "├", "┼", "┤", rule));
            }
        }
        self.out.push(border(&widths, "└", "┴", "┘", rule));
        self.need_blank = true;
    }
}

fn border(widths: &[usize], left: &str, mid: &str, right: &str, style: Style) -> Line<'static> {
    let inner: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    Line::from(Span::styled(
        format!("{left}{}{right}", inner.join(mid)),
        style,
    ))
}

/// 표시 폭 `w`에 맞춰 자르고(…) 오른쪽을 공백으로 채운다.
fn fit(s: &str, w: usize) -> String {
    let sw = s.width();
    if sw <= w {
        return format!("{s}{}", " ".repeat(w - sw));
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if used + cw + 1 > w {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    used += 1;
    out.push_str(&" ".repeat(w.saturating_sub(used)));
    out
}

fn file_name(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("image")
        .to_string()
}

fn width_of(chars: &[(char, Style)]) -> usize {
    chars.iter().map(|(c, _)| c.width().unwrap_or(0)).sum()
}

fn make_line(prefix: &[Seg], chars: &[(char, Style)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = prefix
        .iter()
        .map(|(t, s)| Span::styled(t.clone(), *s))
        .collect();
    let mut buf = String::new();
    let mut style: Option<Style> = None;
    for (c, st) in chars {
        if style != Some(*st) {
            if let Some(prev) = style
                && !buf.is_empty()
            {
                spans.push(Span::styled(std::mem::take(&mut buf), prev));
            }
            style = Some(*st);
        }
        buf.push(*c);
    }
    if let Some(st) = style
        && !buf.is_empty()
    {
        spans.push(Span::styled(buf, st));
    }
    Line::from(spans)
}

/// 표시 폭 기준 줄바꿈. 보통은 마지막 공백에서 끊고, 공백이 없으면 글자 단위로 끊는다.
/// `code`이면 글자 단위로만 끊고 줄 앞 공백을 지우지 않는다.
fn wrap(segs: &[Seg], first: &[Seg], rest: &[Seg], width: usize, code: bool) -> Vec<Line<'static>> {
    let prefix_w = |p: &[Seg]| p.iter().map(|(t, _)| t.width()).sum::<usize>();
    let avail_first = width.saturating_sub(prefix_w(first)).max(1);
    let avail_rest = width.saturating_sub(prefix_w(rest)).max(1);
    let mut lines = Vec::new();
    let mut cur: Vec<(char, Style)> = Vec::new();
    let mut on_first = true;
    for (text, style) in segs {
        for c in text.chars() {
            if c == '\n' {
                lines.push(make_line(if on_first { first } else { rest }, &cur));
                cur.clear();
                on_first = false;
                continue;
            }
            let cw = c.width().unwrap_or(0);
            loop {
                let avail = if on_first { avail_first } else { avail_rest };
                if cur.is_empty() || width_of(&cur) + cw <= avail {
                    break;
                }
                let prefix = if on_first { first } else { rest };
                let space = if code {
                    None
                } else {
                    cur.iter()
                        .rposition(|(ch, _)| *ch == ' ')
                        .filter(|&i| i > 0)
                };
                match space {
                    Some(i) => {
                        let tail = cur.split_off(i + 1);
                        cur.pop();
                        lines.push(make_line(prefix, &cur));
                        cur = tail;
                    }
                    None => {
                        lines.push(make_line(prefix, &cur));
                        cur.clear();
                    }
                }
                on_first = false;
            }
            if c == ' ' && cur.is_empty() && !on_first && !code {
                continue;
            }
            cur.push((c, *style));
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(make_line(if on_first { first } else { rest }, &cur));
    }
    lines
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test markdown`
Expected: `test result: ok. 22 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/markdown.rs
git commit -m "feat(markdown): 터미널용 markdown 렌더러" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 10: 조회 CLI (`login`, `logout`, `whoami`, `mine`, `search`, `show`)

모든 명령은 같은 순서로 동작한다. 먼저 API를 부르고, 받은 것을 캐시에 저장한다. 오프라인이면 캐시로 대신 보여준다(스펙 6.3장 SWR의 CLI판).
- 명령 함수는 출력할 문자열을 돌려준다. 그래서 테스트에서는 출력 문자열을 그대로 비교할 수 있다.
- 오프라인 상황은 아무도 듣지 않는 주소(`127.0.0.1:9`)로 흉내 낸다.
- 표준 출력이 터미널일 때만 색(ANSI)을 쓴다.
- `HERDR_LINEAR_DEBUG=1`이면 마지막 응답의 한도 헤더를 stderr에 찍는다(Task 11에서 복잡도를 잴 때 쓴다).

**Files:**
- Create: `src/cli.rs`, `src/main.rs`
- Modify: `src/lib.rs`
- Test: `src/cli.rs`의 `tests` 모듈

**Interfaces:**
- Consumes: Task 1~9의 공개 함수 전부(`config::*`, `LinearClient`, `queries::*`, `build_issue_filter`, `token_filter`, `parse`, `SearchIndex`, `merge`, `sort_mine`, `Store`, `markdown::{render, to_plain, to_ansi, sanitize, Theme}`)
- Produces (2부에서 재사용):
  - `cli::Cli`, `cli::Command::{Login, Logout, Whoami, Mine, Search { deep: bool, query: Vec<String> }, Show { id: String }}`
  - `cli::Ctx { paths, settings, store, client, now_ms: i64, color: bool, width: u16 }`, `Ctx::open(Paths) -> Result<Ctx>`
  - `cli::run(Cli) -> Result<String>`, `cli::now_ms() -> i64`
  - `cli::login(&Paths)`, `cli::login_with_key(&Paths, key: &str, make_client: impl Fn(String) -> LinearClient) -> Result<String>`, `cli::logout(&Paths) -> Result<String>`
  - `cli::load_viewer(&Ctx, force: bool) -> Result<Option<Viewer>>`(60분 캐시, 오프라인이면 오래된 캐시)
  - `cli::scope_team_ids(&Viewer, &Settings) -> Vec<String>`
  - `cli::whoami`, `cli::mine`(`&Ctx -> Result<String>`), `cli::search(&Ctx, input: &str, deep: bool)`, `cli::show(&Ctx, id: &str)`
  - `cli::state_icon(&str) -> &'static str`, `cli::priority_label(i64) -> &'static str`, `cli::ago(now_ms, then_ms) -> String`, `cli::issue_line(&Issue) -> String`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/lib.rs` 전체(최종):

````rust
//! herdr-linear: herdr 안에서 Linear를 빠르게 조회한다.

pub mod cli;
pub mod config;
pub mod linear;
pub mod markdown;
pub mod search;
pub mod store;

#[cfg(test)]
pub mod test_support;
````

`src/cli.rs` (테스트 모듈만 먼저):

````rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::IssueBuilder;
    use mockito::Matcher;
    use serde_json::{Value, json};

    const NOW: i64 = 1_791_288_000_000;

    fn test_ctx(endpoint: String) -> (tempfile::TempDir, Ctx) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            state_dir: dir.path().join("state"),
        };
        let ctx = Ctx {
            paths,
            settings: Settings::default(),
            store: Store::open_in_memory().unwrap(),
            client: LinearClient::with_endpoint("lin_api_test", endpoint),
            now_ms: NOW,
            color: false,
            width: 60,
        };
        (dir, ctx)
    }

    fn url(server: &mockito::Server) -> String {
        format!("{}/graphql", server.url())
    }

    /// 아무도 듣지 않는 주소 (오프라인 흉내)
    const OFFLINE: &str = "http://127.0.0.1:9/graphql";

    fn viewer_json() -> Value {
        json!({
            "id": "me", "name": "김민수", "displayName": "minsu", "email": "m@acme.dev",
            "organization": { "id": "org1", "name": "Acme", "urlKey": "acme" },
            "teams": { "nodes": [
                { "id": "team-ENG", "key": "ENG", "name": "Engineering" },
                { "id": "team-OPS", "key": "OPS", "name": "Ops" }
            ] }
        })
    }

    fn mock_viewer(server: &mut mockito::Server) -> mockito::Mock {
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Viewer".into()))
            .with_body(json!({ "data": { "viewer": viewer_json() } }).to_string())
            .create()
    }

    fn mock_issues(server: &mut mockito::Server, nodes: Vec<Value>) -> mockito::Mock {
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Issues".into()))
            .with_body(
                json!({ "data": { "issues": { "nodes": nodes, "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
                    .to_string(),
            )
            .create()
    }

    #[test]
    fn cli_parses_search_with_deep_flag() {
        let cli =
            Cli::try_parse_from(["herdr-linear", "search", "--deep", "세션", "만료"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Search {
                deep: true,
                query: vec!["세션".into(), "만료".into()]
            }
        );
        assert!(Cli::try_parse_from(["herdr-linear", "search"]).is_err());
    }

    #[test]
    fn mine_sorts_and_saves_view() {
        let mut server = mockito::Server::new();
        mock_issues(
            &mut server,
            vec![
                IssueBuilder::new("i2", "ENG-2", "할 일").json(),
                IssueBuilder::new("i1", "ENG-1", "진행 중")
                    .state("In Progress", "started")
                    .labels(&["bug"])
                    .json(),
            ],
        );
        let (_d, ctx) = test_ctx(url(&server));
        let out = mine(&ctx).unwrap();
        assert_eq!(out, "◐ ENG-1     진행 중  [bug]\n○ ENG-2     할 일");
        let (saved, _) = ctx.store.get_view("mine").unwrap().unwrap();
        assert_eq!(
            saved
                .iter()
                .map(|i| i.identifier.as_str())
                .collect::<Vec<_>>(),
            vec!["ENG-1", "ENG-2"]
        );
    }

    #[test]
    fn mine_offline_uses_saved_view() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("i1", "ENG-1", "저장된 이슈").build()],
                NOW,
            )
            .unwrap();
        ctx.store
            .set_view("mine", &["i1".into()], NOW - 5 * 60_000)
            .unwrap();
        let out = mine(&ctx).unwrap();
        assert!(out.starts_with("(오프라인: 5분 전 저장된 결과"), "{out}");
        assert!(out.contains("ENG-1"));
    }

    #[test]
    fn mine_offline_without_cache_is_error() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        assert!(mine(&ctx).is_err());
    }

    #[test]
    fn search_merges_local_and_server() {
        let mut server = mockito::Server::new();
        mock_viewer(&mut server);
        let m = server
            .mock("POST", "/graphql")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("query Issues".into()),
                Matcher::PartialJson(json!({ "variables": { "filter": { "and": [
                    { "or": [ { "title": { "containsIgnoreCase": "로그인" } }, { "description": { "containsIgnoreCase": "로그인" } } ] },
                    { "team": { "id": { "in": ["team-ENG", "team-OPS"] } } }
                ] } } })),
            ]))
            .with_body(
                json!({ "data": { "issues": { "nodes": [ IssueBuilder::new("s1", "ENG-1", "로그인 버튼").json() ], "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
                    .to_string(),
            )
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("l1", "OPS-9", "로그인 서버").build()],
                NOW,
            )
            .unwrap();
        let out = search(&ctx, "로그인", false).unwrap();
        assert!(out.contains("ENG-1     로그인 버튼"), "{out}");
        assert!(out.contains("OPS-9     로그인 서버"), "{out}");
        m.assert();
        // 서버 결과는 캐시에 저장된다
        assert!(ctx.store.get_issue("ENG-1").unwrap().is_some());
    }

    #[test]
    fn search_offline_falls_back_to_local() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("l1", "OPS-9", "로그인 서버").build()],
                NOW,
            )
            .unwrap();
        let out = search(&ctx, "로그인", false).unwrap();
        assert!(
            out.starts_with("(오프라인: 저장된 이슈에서만 찾았어요"),
            "{out}"
        );
        assert!(out.contains("OPS-9"));
    }

    #[test]
    fn deep_search_needs_text() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        assert!(search(&ctx, "l:bug", true).is_err());
    }

    #[test]
    fn show_renders_detail_and_marks_viewed() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "로그인 버그")
            .state("In Progress", "started")
            .priority(2)
            .assignee("me", "김민수")
            .labels(&["bug"])
            .description("## 재현\n- 로그인 후 대기\n\n[로그](https://x.dev/log)")
            .json();
        issue["comments"] = json!({
            "nodes": [ { "id": "c1", "body": "확인할게요", "createdAt": "2026-10-01T00:00:00.000Z", "editedAt": null,
                         "user": { "id": "u2", "name": "이영희", "displayName": "영희" } } ],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        });
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex("query Detail".into()))
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        let out = show(&ctx, "ENG-1").unwrap();
        assert!(
            out.starts_with("ENG-1  로그인 버그\n◐ In Progress · 우선순위 높음 · @김민수\nbug\n"),
            "{out}"
        );
        assert!(out.contains("• 로그인 후 대기"), "{out}");
        assert!(out.contains("로그 [1]"), "{out}");
        assert!(out.contains("[1] https://x.dev/log"), "{out}");
        assert!(out.contains("── 코멘트 1 ──"), "{out}");
        assert!(out.contains("영희 · "), "{out}");
        assert!(out.contains("확인할게요"), "{out}");
        assert_eq!(ctx.store.recent_viewed(5).unwrap().len(), 1);
        assert!(ctx.store.get_comments("i1").unwrap().is_some());
    }

    #[test]
    fn show_not_found_drops_cached_copy() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"code":"INVALID_INPUT"}}]}"#)
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        ctx.store
            .upsert_issues(&[IssueBuilder::new("i9", "ENG-9", "옛 이슈").build()], NOW)
            .unwrap();
        assert!(show(&ctx, "ENG-9").is_err());
        assert_eq!(ctx.store.get_issue("ENG-9").unwrap(), None);
    }

    #[test]
    fn show_archived_says_gone() {
        let mut server = mockito::Server::new();
        let mut issue = IssueBuilder::new("i1", "ENG-1", "보관됨").archived().json();
        issue["comments"] =
            json!({ "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } });
        server
            .mock("POST", "/graphql")
            .with_body(json!({ "data": { "issue": issue } }).to_string())
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        assert_eq!(
            show(&ctx, "ENG-1").unwrap(),
            "ENG-1은(는) 보관되었거나 삭제된 이슈예요"
        );
    }

    #[test]
    fn show_offline_uses_cache() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store
            .upsert_issues(
                &[IssueBuilder::new("i1", "ENG-1", "저장된 상세")
                    .description("본문")
                    .build()],
                NOW,
            )
            .unwrap();
        let out = show(&ctx, "eng-1").unwrap();
        assert!(out.starts_with("(오프라인: 저장된 내용"), "{out}");
        assert!(out.contains("ENG-1  저장된 상세"));
        assert!(out.contains("본문"));
    }

    #[test]
    fn login_saves_key_and_viewer() {
        let mut server = mockito::Server::new();
        mock_viewer(&mut server);
        let (_d, ctx) = test_ctx(url(&server));
        let endpoint = url(&server);
        let out = login_with_key(&ctx.paths, "  lin_api_new  ", |k| {
            LinearClient::with_endpoint(k, endpoint.clone())
        })
        .unwrap();
        assert_eq!(out, "김민수님, Acme 워크스페이스에 연결됐어요");
        assert_eq!(
            std::fs::read_to_string(ctx.paths.credentials_file()).unwrap(),
            "lin_api_new\n"
        );
        let store = Store::open(&ctx.paths.cache_db()).unwrap();
        assert_eq!(store.meta_get("org_id").unwrap().as_deref(), Some("org1"));
    }

    #[test]
    fn invalid_key_saves_nothing() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#)
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        let endpoint = url(&server);
        let err = login_with_key(&ctx.paths, "bad", |k| {
            LinearClient::with_endpoint(k, endpoint.clone())
        })
        .unwrap_err();
        assert!(err.to_string().contains("유효하지 않아요"));
        assert!(!ctx.paths.credentials_file().exists());
    }

    #[test]
    fn logout_removes_key_and_cache() {
        let (_d, ctx) = test_ctx(OFFLINE.into());
        config::save_api_key(&ctx.paths, "lin_api_x").unwrap();
        Store::open(&ctx.paths.cache_db()).unwrap();
        logout(&ctx.paths).unwrap();
        assert!(!ctx.paths.credentials_file().exists());
        assert!(!ctx.paths.cache_db().exists());
    }

    #[test]
    fn scope_follows_settings() {
        let v: Viewer = serde_json::from_value(viewer_json()).unwrap();
        assert_eq!(
            scope_team_ids(&v, &Settings::default()),
            vec!["team-ENG", "team-OPS"]
        );
        let only_ops = Settings {
            teams: vec!["ops".into()],
            ..Settings::default()
        };
        assert_eq!(scope_team_ids(&v, &only_ops), vec!["team-OPS"]);
    }

    #[test]
    fn viewer_cache_is_used_within_ttl() {
        let mut server = mockito::Server::new();
        let m = mock_viewer(&mut server).expect(1);
        let (_d, ctx) = test_ctx(url(&server));
        load_viewer(&ctx, false).unwrap().unwrap();
        load_viewer(&ctx, false).unwrap().unwrap();
        m.assert();
    }

    #[test]
    fn list_and_detail_strip_control_characters() {
        let issue = IssueBuilder::new("i1", "ENG-1", "로그인\u{1b}[2J 버그")
            .assignee("u1", "민수\u{1b}]0;x\u{7}")
            .build();
        assert_eq!(issue_line(&issue), "○ ENG-1     로그인[2J 버그  @민수]0;x");
        let (_d, ctx) = test_ctx(OFFLINE.into());
        ctx.store.upsert_issues(&[issue], NOW).unwrap();
        let out = show(&ctx, "ENG-1").unwrap();
        assert!(!out.contains('\u{1b}'), "{out:?}");
        assert!(!out.contains('\u{7}'), "{out:?}");
    }

    #[test]
    fn rate_limit_says_when_to_retry() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_header("x-ratelimit-requests-reset", &(NOW + 5 * 60_000).to_string())
            .with_body(r#"{"errors":[{"message":"Rate limit exceeded","extensions":{"code":"RATELIMITED"}}]}"#)
            .create();
        let (_d, ctx) = test_ctx(url(&server));
        let err = mine(&ctx).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Linear API 한도를 넘었어요. 5분 후 다시 시도하세요"
        );
    }

    #[test]
    fn ago_is_human_readable() {
        assert_eq!(ago(NOW, NOW), "방금");
        assert_eq!(ago(NOW, NOW - 59 * 60_000), "59분 전");
        assert_eq!(ago(NOW, NOW - 3 * 3_600_000), "3시간 전");
        assert_eq!(ago(NOW, NOW - 2 * 86_400_000), "2일 전");
    }
}
````

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cargo test cli::`
Expected: 컴파일 실패 — `cannot find type `Ctx`` 같은 오류

- [ ] **Step 3: 구현 작성**

`src/cli.rs` 맨 위에 넣는다:

````rust
//! 터미널 조회 명령: login, logout, whoami, mine, search, show.

use std::io::IsTerminal;

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};

use crate::config::{self, Paths, Settings};
use crate::linear::client::{ApiError, LinearClient};
use crate::linear::filter::{build_issue_filter, token_filter};
use crate::linear::queries;
use crate::linear::types::{Comment, Issue, Viewer};
use crate::markdown::{self, Theme};
use crate::search::query::parse;
use crate::search::rank::{SearchIndex, merge, sort_mine};
use crate::store::{Store, remove_db_files};

const VIEWER_TTL_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const SEARCH_LIMIT: usize = 30;

#[derive(Parser, Debug)]
#[command(
    name = "herdr-linear",
    version,
    about = "herdr에서 Linear를 빠르게 조회"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Command {
    /// API 키를 입력하고 검증해 저장한다
    Login,
    /// 저장된 API 키와 캐시를 지운다
    Logout,
    /// 연결된 계정과 워크스페이스
    Whoami,
    /// 나에게 할당된 열린 이슈
    Mine,
    /// 이슈 검색 (예: 로그인 l:bug s:진행 @나 #ENG p:high)
    Search {
        /// 서버 깊은 검색 (코멘트 포함, 분당 30회 제한)
        #[arg(long)]
        deep: bool,
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
    },
    /// 이슈 상세 (예: ENG-131)
    Show { id: String },
}

/// 명령 실행에 필요한 것들.
pub struct Ctx {
    pub paths: Paths,
    pub settings: Settings,
    pub store: Store,
    pub client: LinearClient,
    pub now_ms: i64,
    /// 색을 쓸지 (표준 출력이 터미널일 때)
    pub color: bool,
    /// markdown 렌더링 폭
    pub width: u16,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Ctx {
    pub fn open(paths: Paths) -> Result<Ctx> {
        let (settings, warnings) = config::load_settings(&paths.config_file());
        for w in warnings {
            eprintln!("경고: {w}");
        }
        let key = config::resolve_api_key(std::env::var("LINEAR_API_KEY").ok(), &paths)?
            .ok_or_else(|| anyhow!("API 키가 없어요. 먼저 `herdr-linear login`을 실행하세요"))?;
        let store = Store::open(&paths.cache_db())?;
        let now = now_ms();
        store.evict_older_than(now - settings.cache_retention_days as i64 * DAY_MS)?;
        let width = ratatui::crossterm::terminal::size()
            .map(|(w, _)| w)
            .unwrap_or(100)
            .min(120);
        Ok(Ctx {
            paths,
            settings,
            store,
            client: LinearClient::new(key.value),
            now_ms: now,
            color: std::io::stdout().is_terminal(),
            width,
        })
    }
}

/// 명령을 실행하고 출력할 문자열을 돌려준다.
pub fn run(cli: Cli) -> Result<String> {
    let paths = Paths::from_env()?;
    match cli.command {
        Command::Login => return login(&paths),
        Command::Logout => return logout(&paths),
        _ => {}
    }
    let ctx = Ctx::open(paths)?;
    let out = match cli.command {
        Command::Whoami => whoami(&ctx),
        Command::Mine => mine(&ctx),
        Command::Search { deep, query } => search(&ctx, &query.join(" "), deep),
        Command::Show { id } => show(&ctx, &id),
        Command::Login | Command::Logout => Ok(String::new()),
    };
    if std::env::var_os("HERDR_LINEAR_DEBUG").is_some() {
        let r = ctx.client.rate_limit();
        eprintln!(
            "[debug] remaining={:?} complexity={:?} reset_ms={:?}",
            r.requests_remaining, r.complexity, r.requests_reset_ms
        );
    }
    out
}

pub fn login(paths: &Paths) -> Result<String> {
    let key = rpassword::prompt_password(
        "Linear API 키 (Linear → Settings → Security & access → Personal API keys): ",
    )?;
    let mut out = login_with_key(paths, &key, LinearClient::new)?;
    if std::env::var("LINEAR_API_KEY").is_ok_and(|v| !v.trim().is_empty()) {
        out.push_str("\n참고: LINEAR_API_KEY 환경 변수가 설정돼 있어서 그 값이 우선 쓰여요");
    }
    Ok(out)
}

/// 키를 `viewer` 쿼리로 검증하고, 맞으면 저장한다.
pub fn login_with_key(
    paths: &Paths,
    key: &str,
    make_client: impl Fn(String) -> LinearClient,
) -> Result<String> {
    let key = key.trim();
    if key.is_empty() {
        bail!("키가 비어 있어요");
    }
    let client = make_client(key.to_string());
    let viewer = match queries::viewer(&client) {
        Ok(v) => v,
        Err(ApiError::Auth) => bail!("키가 유효하지 않아요. Linear에서 키를 다시 확인하세요"),
        Err(e) => return Err(e.into()),
    };
    config::save_api_key(paths, key)?;
    let store = Store::open(&paths.cache_db())?;
    save_viewer(&store, &viewer, now_ms())?;
    Ok(format!(
        "{}님, {} 워크스페이스에 연결됐어요",
        viewer.name, viewer.organization.name
    ))
}

pub fn logout(paths: &Paths) -> Result<String> {
    config::delete_credentials(paths)?;
    remove_db_files(&paths.cache_db());
    Ok("API 키와 캐시를 지웠어요".to_string())
}

fn save_viewer(store: &Store, v: &Viewer, now: i64) -> Result<()> {
    store.ensure_org(&v.organization.id)?;
    store.meta_set("viewer", &serde_json::to_string(v)?)?;
    store.meta_set("viewer_at", &now.to_string())?;
    Ok(())
}

/// 내 정보. 60분 안에 받은 것이 있으면 캐시를 쓴다.
/// 오프라인이면 오래된 캐시라도 쓰고, 그것도 없으면 `None`.
pub fn load_viewer(ctx: &Ctx, force: bool) -> Result<Option<Viewer>> {
    let cached: Option<(Viewer, i64)> = match (
        ctx.store.meta_get("viewer")?,
        ctx.store.meta_get("viewer_at")?,
    ) {
        (Some(v), Some(at)) => serde_json::from_str(&v)
            .ok()
            .map(|v| (v, at.parse().unwrap_or(0))),
        _ => None,
    };
    if !force
        && let Some((v, at)) = &cached
        && ctx.now_ms - at < VIEWER_TTL_MS
    {
        return Ok(Some(v.clone()));
    }
    match queries::viewer(&ctx.client) {
        Ok(v) => {
            save_viewer(&ctx.store, &v, ctx.now_ms)?;
            Ok(Some(v))
        }
        Err(ApiError::Offline(_)) => Ok(cached.map(|(v, _)| v)),
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

/// 검색 범위 팀 id. config의 `teams`가 비어 있으면 내가 속한 팀 전부.
pub fn scope_team_ids(viewer: &Viewer, settings: &Settings) -> Vec<String> {
    viewer
        .teams
        .nodes
        .iter()
        .filter(|t| {
            settings.teams.is_empty()
                || settings
                    .teams
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(&t.key))
        })
        .map(|t| t.id.clone())
        .collect()
}

pub fn whoami(ctx: &Ctx) -> Result<String> {
    let v = load_viewer(ctx, true)?
        .ok_or_else(|| anyhow!("오프라인이라 계정 정보를 가져올 수 없어요"))?;
    let teams: Vec<String> = v
        .teams
        .nodes
        .iter()
        .map(|t| format!("{} ({})", t.key, t.name))
        .collect();
    let mut out = vec![
        format!("{} ({}) <{}>", v.name, v.display_name, v.email),
        format!(
            "워크스페이스: {} ({})",
            v.organization.name, v.organization.url_key
        ),
        format!("팀: {}", teams.join(", ")),
        format!(
            "검색 범위: 팀 {}개",
            scope_team_ids(&v, &ctx.settings).len()
        ),
    ];
    let rate = ctx.client.rate_limit();
    if let Some(r) = rate.requests_remaining {
        out.push(format!("남은 요청: {r} (시간당)"));
    }
    Ok(out.join("\n"))
}

pub fn mine(ctx: &Ctx) -> Result<String> {
    match queries::my_issues(&ctx.client) {
        Ok(issues) => {
            ctx.store.upsert_issues(&issues, ctx.now_ms)?;
            let mut issues: Vec<Issue> = issues.into_iter().filter(|i| !i.is_gone()).collect();
            sort_mine(&mut issues);
            let ids: Vec<String> = issues.iter().map(|i| i.id.clone()).collect();
            ctx.store.set_view("mine", &ids, ctx.now_ms)?;
            Ok(format_list(&issues, None))
        }
        Err(ApiError::Offline(msg)) => match ctx.store.get_view("mine")? {
            Some((issues, at)) => Ok(format_list(
                &issues,
                Some(&format!(
                    "오프라인: {} 저장된 결과 · {msg}",
                    ago(ctx.now_ms, at)
                )),
            )),
            None => Err(anyhow!("오프라인이고 저장된 결과도 없어요: {msg}")),
        },
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

pub fn search(ctx: &Ctx, input: &str, deep: bool) -> Result<String> {
    let q = parse(input);
    if deep && q.text().is_empty() {
        bail!("깊은 검색에는 검색어가 필요해요");
    }
    let viewer = load_viewer(ctx, false)?;
    let viewer_id = viewer.as_ref().map(|v| v.id.as_str());
    let local = SearchIndex::new(ctx.store.all_issues()?).search(&q, viewer_id);
    let server = if deep {
        queries::deep_search(&ctx.client, &q.text(), token_filter(&q).as_ref())
    } else {
        let scope = viewer
            .as_ref()
            .map(|v| scope_team_ids(v, &ctx.settings))
            .unwrap_or_default();
        queries::filter_issues(&ctx.client, &build_issue_filter(&q, &scope))
    };
    match server {
        Ok(found) => {
            ctx.store.upsert_issues(&found, ctx.now_ms)?;
            let found: Vec<Issue> = found.into_iter().filter(|i| !i.is_gone()).collect();
            let merged = merge(&local, &found, &q, viewer_id);
            Ok(format_list(&merged[..merged.len().min(SEARCH_LIMIT)], None))
        }
        Err(ApiError::Offline(msg)) => Ok(format_list(
            &local[..local.len().min(SEARCH_LIMIT)],
            Some(&format!("오프라인: 저장된 이슈에서만 찾았어요 · {msg}")),
        )),
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

pub fn show(ctx: &Ctx, id: &str) -> Result<String> {
    match queries::issue_detail(&ctx.client, id) {
        Ok(Some(d)) => {
            let removed = ctx
                .store
                .upsert_issues(std::slice::from_ref(&d.issue), ctx.now_ms)?;
            if !removed.is_empty() {
                return Ok(format!(
                    "{}은(는) 보관되었거나 삭제된 이슈예요",
                    d.issue.identifier
                ));
            }
            ctx.store
                .set_comments(&d.issue.id, &d.comments, ctx.now_ms)?;
            ctx.store.mark_viewed(&d.issue.id, ctx.now_ms)?;
            Ok(format_detail(
                ctx,
                &d.issue,
                &d.comments,
                d.more_comments,
                None,
            ))
        }
        Ok(None) => {
            if let Some(cached) = ctx.store.get_issue(id)? {
                ctx.store.remove_issue(&cached.id)?;
            }
            bail!("{id} 이슈를 찾을 수 없어요 (보관·삭제됐거나 권한이 없을 수 있어요)")
        }
        Err(ApiError::Offline(msg)) => {
            let issue = ctx
                .store
                .get_issue(id)?
                .ok_or_else(|| anyhow!("오프라인이고 저장된 {id}도 없어요: {msg}"))?;
            let comments = ctx
                .store
                .get_comments(&issue.id)?
                .map(|(c, _)| c)
                .unwrap_or_default();
            Ok(format_detail(
                ctx,
                &issue,
                &comments,
                false,
                Some(&format!("오프라인: 저장된 내용 · {msg}")),
            ))
        }
        Err(e) => Err(api_error(e, ctx.now_ms)),
    }
}

pub fn state_icon(state_type: &str) -> &'static str {
    match state_type {
        "triage" => "◇",
        "backlog" => "◌",
        "unstarted" => "○",
        "started" => "◐",
        "completed" => "●",
        "canceled" | "duplicate" => "✕",
        _ => "·",
    }
}

pub fn priority_label(p: i64) -> &'static str {
    match p {
        1 => "긴급",
        2 => "높음",
        3 => "보통",
        4 => "낮음",
        _ => "없음",
    }
}

pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let mins = (now_ms - then_ms).max(0) / 60_000;
    match mins {
        0 => "방금".to_string(),
        m if m < 60 => format!("{m}분 전"),
        m if m < 60 * 24 => format!("{}시간 전", m / 60),
        m => format!("{}일 전", m / (60 * 24)),
    }
}

/// 한도 초과면 언제 다시 시도할 수 있는지 알려준다.
fn api_error(e: ApiError, now_ms: i64) -> anyhow::Error {
    match e {
        ApiError::RateLimited {
            reset_at_ms: Some(reset),
        } => {
            let mins = ((reset - now_ms).max(0) + 59_999) / 60_000;
            anyhow!("Linear API 한도를 넘었어요. {mins}분 후 다시 시도하세요")
        }
        other => other.into(),
    }
}

pub fn issue_line(issue: &Issue) -> String {
    let mut s = format!(
        "{} {:<9} {}",
        state_icon(&issue.state.state_type),
        issue.identifier,
        issue.title
    );
    let labels = issue.label_names();
    if !labels.is_empty() {
        s.push_str(&format!("  [{}]", labels.join(", ")));
    }
    if let Some(a) = &issue.assignee {
        s.push_str(&format!("  @{}", a.display_name));
    }
    markdown::sanitize(&s)
}

fn format_list(issues: &[Issue], banner: Option<&str>) -> String {
    let mut out = Vec::new();
    if let Some(b) = banner {
        out.push(format!("({b})"));
    }
    if issues.is_empty() {
        out.push("결과가 없어요".to_string());
    }
    out.extend(issues.iter().map(issue_line));
    out.join("\n")
}

fn format_detail(
    ctx: &Ctx,
    issue: &Issue,
    comments: &[Comment],
    more: bool,
    banner: Option<&str>,
) -> String {
    let theme = Theme::default();
    let mut out = Vec::new();
    if let Some(b) = banner {
        out.push(format!("({b})"));
    }
    out.push(format!("{}  {}", issue.identifier, issue.title));
    out.push(
        [
            format!(
                "{} {}",
                state_icon(&issue.state.state_type),
                issue.state.name
            ),
            format!("우선순위 {}", priority_label(issue.priority)),
            match &issue.assignee {
                Some(a) => format!("@{}", a.display_name),
                None => "담당자 없음".to_string(),
            },
        ]
        .join(" · "),
    );
    let mut extra = Vec::new();
    let labels = issue.label_names();
    if !labels.is_empty() {
        extra.push(labels.join(", "));
    }
    if let Some(p) = &issue.project {
        extra.push(format!("프로젝트 {}", p.name));
    }
    if let Some(c) = &issue.cycle {
        extra.push(format!(
            "사이클 {}",
            c.name
                .clone()
                .unwrap_or_else(|| (c.number as i64).to_string())
        ));
    }
    if let Some(p) = &issue.parent {
        extra.push(format!("상위 {}", p.identifier));
    }
    if let Some(e) = issue.estimate {
        extra.push(format!("예상 {e}"));
    }
    if let Some(d) = &issue.due_date {
        extra.push(format!("마감 {d}"));
    }
    if !extra.is_empty() {
        out.push(extra.join(" · "));
    }
    out.push(issue.url.clone());
    // 여기까지는 Linear 값이 그대로 들어간 평문이라 제어 문자를 지운다
    let mut out: Vec<String> = out.iter().map(|l| markdown::sanitize(l)).collect();
    out.push(String::new());
    let body = issue.description.as_deref().unwrap_or("").trim();
    if body.is_empty() {
        out.push("(본문 없음)".to_string());
    } else {
        out.push(render_md(ctx, body, &theme));
    }
    if !comments.is_empty() {
        out.push(String::new());
        out.push(format!(
            "── 코멘트 {}{} ──",
            comments.len(),
            if more { "+" } else { "" }
        ));
        for c in comments {
            let who = c
                .user
                .as_ref()
                .map(|u| u.display_name.as_str())
                .unwrap_or("알 수 없음");
            out.push(String::new());
            out.push(markdown::sanitize(&format!(
                "{who} · {}",
                short_time(&c.created_at)
            )));
            out.push(render_md(ctx, &c.body, &theme));
        }
        if more {
            out.push(String::new());
            out.push("코멘트가 더 있어요. 브라우저에서 보세요".to_string());
        }
    }
    out.join("\n")
}

fn render_md(ctx: &Ctx, md: &str, theme: &Theme) -> String {
    let r = markdown::render(md, ctx.width, theme);
    let mut text = if ctx.color {
        markdown::to_ansi(&r.lines)
    } else {
        markdown::to_plain(&r.lines)
    };
    if !r.links.is_empty() {
        text.push('\n');
        for l in &r.links {
            text.push_str(&format!("\n[{}] {}", l.index, l.url));
        }
    }
    text
}

fn short_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| rfc3339.to_string())
}
````

`src/main.rs`:

````rust
use clap::Parser;

fn main() {
    let cli = herdr_linear::cli::Cli::parse();
    match herdr_linear::cli::run(cli) {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
        }
        Err(e) => {
            eprintln!("오류: {e:#}");
            std::process::exit(1);
        }
    }
}
````

- [ ] **Step 4: 테스트가 통과하는지 확인**

Run: `cargo test cli::`
Expected: `test result: ok. 19 passed`

이어서 전체 점검:

````bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
````

Expected: clippy 경고 0개, 모든 테스트 통과.

키 없이 실행했을 때의 동작도 확인한다:

````bash
cargo build
HOME=$(mktemp -d) ./target/debug/herdr-linear mine; echo "exit=$?"
````

Expected: `오류: API 키가 없어요. 먼저 `herdr-linear login`을 실행하세요`, `exit=1`

- [ ] **Step 5: 커밋**

````bash
git add src/lib.rs src/cli.rs src/main.rs
git commit -m "feat(cli): 조회 명령 login·logout·whoami·mine·search·show" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````

---

### Task 11: 실제 Linear로 확인하고 결과 기록

**사람이 해야 하는 단계가 있다.** `login`의 키 입력은 숨김 입력이라 에이전트가 대신할 수 없다. Step 2는 사람이 직접 하고, 나머지 단계는 에이전트가 해도 된다. 이 작업은 스펙 15장 가정 중 Linear 쪽 항목의 답을 기록한다. 2부와 3부 계획이 이 기록을 입력으로 쓴다.

**Files:**
- Create: `docs/superpowers/notes/2026-10-06-linear-api-checks.md`

**Interfaces:**
- Consumes: Task 10의 `herdr-linear` 바이너리
- Produces: 실측 결과 문서(2부·3부 계획의 입력)

- [ ] **Step 1: release 빌드**

````bash
cargo build --release
./target/release/herdr-linear --help
````

Expected: `login`, `logout`, `whoami`, `mine`, `search`, `show` 6개 명령이 보인다.

- [ ] **Step 2: (사람) 키 발급과 로그인**

Linear → Settings → Security & access → Personal API keys에서 키를 만든다. 그다음 실행한다:

````bash
./target/release/herdr-linear login
````

Expected: `<이름>님, <워크스페이스> 워크스페이스에 연결됐어요`

- [ ] **Step 3: 내 정보와 한도**

````bash
HERDR_LINEAR_DEBUG=1 ./target/release/herdr-linear whoami
````

Expected: 이름·워크스페이스·팀 목록이 출력되고, stderr에 `[debug] remaining=Some(..)`가 찍힌다.

- [ ] **Step 4: 내 이슈와 쿼리 복잡도**

````bash
HERDR_LINEAR_DEBUG=1 ./target/release/herdr-linear mine
````

Expected: 진행 중 → 할 일 순으로 정렬된 목록. stderr의 `complexity=Some(n)`을 기록하고 5,000 미만인지 본다(스펙 7.2). 넘으면 `queries.rs`의 `labels(first: 20)`을 10으로 줄이고 다시 잰다.

- [ ] **Step 5: 한글 서버 검색**

워크스페이스에 실제로 있는 한글 단어로 검색한다. 단, 캐시에 없는 이슈여야 한다(처음 보는 이슈).

````bash
./target/release/herdr-linear search <한글 단어>
````

Expected: 서버 결과가 나온다. 나오지 않으면 `containsIgnoreCase`가 한글에서 안 되는 것이다. 이 사실을 기록한다(스펙 15장 대안: 로컬 결과만 쓰고 깊은 검색을 안내).

- [ ] **Step 6: 상세와 markdown**

제목, 목록, 코드, 표, 이미지가 있는 이슈 하나를 연다:

````bash
./target/release/herdr-linear show <ID>
````

Expected: 본문이 렌더링되고, 링크·이미지가 `[n]` 번호와 URL 목록으로 보인다. 보관된 이슈 ID로도 한 번 실행해서 메시지가 "보관되었거나 삭제된 이슈"인지 "찾을 수 없어요"인지 기록한다.

- [ ] **Step 7: 깊은 검색과 오프라인**

````bash
./target/release/herdr-linear search --deep <코멘트에만 있는 단어>
````

그다음 네트워크를 끄고 `mine`, `search <단어>`, `show <본 적 있는 ID>`를 실행한다.

Expected: 깊은 검색에서 코멘트가 일치한 이슈가 나온다. 오프라인에서는 `(오프라인: ...)` 안내와 함께 저장된 결과가 나온다.

- [ ] **Step 8: 결과 기록**

`docs/superpowers/notes/2026-10-06-linear-api-checks.md`를 아래 형식으로 만들고, 확인한 결과를 채운다:

````markdown
# Linear API 실측 (1부)

- 날짜:
- 커밋:

| 확인 항목 (스펙 15장) | 결과 | 비고 |
|---|---|---|
| `issues(orderBy: updatedAt)` 정렬 방향 | 해당 없음 | 로컬에서 항상 다시 정렬하므로 결과에 영향 없음 |
| `containsIgnoreCase`가 한글에서 동작하는가 | | 쓴 검색어: |
| `issue(id:)`가 보관 이슈를 어떻게 돌려주는가 | | `archivedAt` 포함 / not found 오류 중 어느 쪽 |
| 50건 페이지 복잡도 (`X-Complexity`) | | 5,000 미만이어야 함 |
| 깊은 검색(코멘트 포함) | | |
| 오프라인 동작 | | |
| markdown 렌더링 (제목·목록·코드·표·이미지) | | 확인한 이슈: |
````

- [ ] **Step 9: 커밋**

````bash
git add docs/superpowers/notes/2026-10-06-linear-api-checks.md
git commit -m "docs: Linear API 실측 결과 (1부)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
````
