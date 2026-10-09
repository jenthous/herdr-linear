//! 설정 파일, 경로, API 키.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::i18n::{Lang, t};

/// herdr 플러그인 id. 액션은 `jh.linear.<액션>`이 되고, herdr가 주는 설정·상태 디렉터리 이름이 된다.
pub const PLUGIN_ID: &str = "jh.linear";
/// 앱 이름. herdr 밖(CLI)에서 쓰는 설정·상태 디렉터리 이름이다.
pub const APP_NAME: &str = "herdr-linear";

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
            .or_else(|| home.as_ref().map(|h| h.join(".config").join(APP_NAME)))
            .ok_or_else(|| anyhow!("{}", t().no_config_dir))?;
        let state_dir = var("HERDR_PLUGIN_STATE_DIR")
            .or_else(|| var("HERDR_LINEAR_STATE_DIR"))
            .or_else(|| {
                home.as_ref()
                    .map(|h| h.join(".local").join("state").join(APP_NAME))
            })
            .ok_or_else(|| anyhow!("{}", t().no_state_dir))?;
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
    /// 화면·CLI 문구의 언어 (`language`). 없거나 모르는 값이면 영어.
    pub language: Lang,
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
            language: Lang::En,
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
/// 읽지 않는 키(철자가 틀린 키, 구역 아래로 들어간 `language`)도 경고한다.
pub fn load_settings(path: &Path) -> (Settings, Vec<String>) {
    let mut s = Settings::default();
    let mut warnings = Vec::new();
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (s, warnings),
        Err(e) => {
            warnings.push((t().config_read_failed)(&e.to_string()));
            return (s, warnings);
        }
    };
    let table: toml::Table = match toml::from_str(&text) {
        Ok(t) => t,
        Err(e) => {
            warnings.push((t().config_invalid)(&e.to_string()));
            return (s, warnings);
        }
    };
    if let Some(v) = table.get("language") {
        match v.as_str() {
            Some(name) => match Lang::parse(name) {
                Some(lang) => s.language = lang,
                None => warnings.push((t().language_unsupported)(name)),
            },
            None => warnings.push(t().language_not_string.to_string()),
        }
    }
    if let Some(v) = table.get("teams") {
        let keys = v.as_array().and_then(|items| {
            items
                .iter()
                .map(|x| x.as_str().map(|k| k.trim().to_uppercase()))
                .collect::<Option<Vec<_>>>()
        });
        match keys {
            Some(keys) => s.teams = keys.into_iter().filter(|k| !k.is_empty()).collect(),
            None => warnings.push(t().teams_not_list.to_string()),
        }
    }
    read_u64(
        &table,
        ("side", "refresh_seconds"),
        0..=3600,
        &mut s.side_refresh_seconds,
        &mut warnings,
    );
    read_u64(
        &table,
        ("cache", "retention_days"),
        1..=3650,
        &mut s.cache_retention_days,
        &mut warnings,
    );
    read_u64(
        &table,
        ("agent", "include_comments"),
        0..=50,
        &mut s.agent_include_comments,
        &mut warnings,
    );
    if let Some(v) = table.get("agent").and_then(|a| a.get("template")) {
        match v.as_str() {
            Some(t) => s.agent_template = t.to_string(),
            None => warnings.push(t().template_not_string.to_string()),
        }
    }
    warn_unread_keys(&table, &mut warnings);
    (s, warnings)
}

/// 읽는 구역(`[side]` 등)의 키. 구역이 아니면 `None`.
/// `load_settings`가 읽는 키와 같아야 한다: 새 키를 읽게 되면 여기에도 더한다.
/// 빠뜨리면 맞는 키가 "알 수 없는 항목"으로 경고된다.
fn section_keys(section: &str) -> Option<&'static [&'static str]> {
    match section {
        "side" => Some(&["refresh_seconds"]),
        "cache" => Some(&["retention_days"]),
        "agent" => Some(&["include_comments", "template"]),
        _ => None,
    }
}

/// 읽지 않고 넘어간 키를 말없이 두지 않고 알린다. 값은 쓰지 않는다.
/// - 구역 안에 들어간 `language`: TOML은 `[구역]` 아래의 키를 그 구역의 키로 읽는다. 맨 위로 옮기라고 알린다.
/// - 모르는 최상위 키와 읽는 구역 안의 모르는 키(`구역.키`): 철자가 틀린 것일 수 있다.
///
/// 키는 `toml::Table`의 순서(이름순)로 훑어서 경고 순서가 늘 같다.
fn warn_unread_keys(table: &toml::Table, warnings: &mut Vec<String>) {
    let t = t();
    for (key, value) in table {
        if matches!(key.as_str(), "language" | "teams") {
            continue;
        }
        let known = section_keys(key);
        if known.is_none() {
            warnings.push((t.unknown_key)(key));
        }
        // 표가 아닌 값(`side = 3` 등)은 안에서 살펴볼 키가 없다
        let Some(inner) = value.as_table() else {
            continue;
        };
        for name in inner.keys() {
            if name == "language" {
                warnings.push((t.language_in_section)(key));
            } else if known.is_some_and(|keys| !keys.contains(&name.as_str())) {
                warnings.push((t.unknown_key)(&format!("{key}.{name}")));
            }
        }
    }
}

/// `[section] key = 정수`를 읽는다. 범위를 벗어나면 기본값을 두고 경고한다.
fn read_u64(
    table: &toml::Table,
    (section, key): (&str, &str),
    range: std::ops::RangeInclusive<u64>,
    target: &mut u64,
    warnings: &mut Vec<String>,
) {
    let Some(v) = table.get(section).and_then(|t| t.get(key)) else {
        return;
    };
    match v.as_integer().and_then(|n| u64::try_from(n).ok()) {
        Some(n) if range.contains(&n) => *target = n,
        _ => warnings.push((t().int_out_of_range)(
            section,
            key,
            *range.start(),
            *range.end(),
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

/// CLI(`~/.config/herdr-linear`)와 herdr 플러그인(`~/.config/herdr/plugins/config/herdr-linear`)은
/// 설정 디렉터리가 다르다. 한쪽에서 로그인한 키를 다른 쪽에서도 찾도록 보조로 읽는 위치.
pub fn credential_fallbacks(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".config").join(APP_NAME).join("credentials"),
        home.join(".config")
            .join("herdr")
            .join("plugins")
            .join("config")
            .join(PLUGIN_ID)
            .join("credentials"),
    ]
}

/// 실제 HOME 기준 보조 위치. 테스트에서는 쓰지 말고 임시 경로를 넘긴다.
pub fn default_credential_fallbacks() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| credential_fallbacks(Path::new(&h)))
        .unwrap_or_default()
}

/// CLI(`~/.local/state/herdr-linear`)와 herdr 플러그인(`~/.local/state/herdr/plugins/jh.linear`)의
/// 캐시 위치. `logout`은 키처럼 두 곳의 캐시를 모두 지운다.
pub fn cache_fallbacks(home: &Path) -> Vec<PathBuf> {
    let state = home.join(".local").join("state");
    vec![
        state.join(APP_NAME).join("cache.db"),
        state
            .join("herdr")
            .join("plugins")
            .join(PLUGIN_ID)
            .join("cache.db"),
    ]
}

/// 실제 HOME 기준 캐시 위치. 테스트에서는 쓰지 말고 임시 경로를 넘긴다.
pub fn default_cache_fallbacks() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| cache_fallbacks(Path::new(&h)))
        .unwrap_or_default()
}

/// 키를 찾는다. 순서: `LINEAR_API_KEY` 환경 변수 → credentials 파일 → `fallbacks`.
pub fn resolve_api_key(
    env_value: Option<String>,
    paths: &Paths,
    fallbacks: &[PathBuf],
) -> Result<Option<ApiKey>> {
    if let Some(v) = env_value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Ok(Some(ApiKey {
            value: v,
            source: KeySource::Env,
        }));
    }
    let primary = paths.credentials_file();
    let candidates = std::iter::once(&primary).chain(fallbacks.iter().filter(|p| **p != primary));
    for path in candidates {
        match fs::read_to_string(path) {
            Ok(text) => {
                let v = text.trim();
                if !v.is_empty() {
                    return Ok(Some(ApiKey {
                        value: v.to_string(),
                        source: KeySource::File,
                    }));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| (t().read_failed)(&path.display().to_string()));
            }
        }
    }
    Ok(None)
}

/// 키를 credentials 파일에 0600 권한으로 저장한다.
pub fn save_api_key(paths: &Paths, key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(anyhow!("{}", t().empty_key));
    }
    ensure_private_dir(&paths.config_dir)?;
    write_private_file(&paths.credentials_file(), format!("{key}\n").as_bytes())
}

/// credentials 파일과 보조 위치의 키를 모두 지운다. 없으면 건너뛴다.
pub fn delete_credentials(paths: &Paths, fallbacks: &[PathBuf]) -> Result<()> {
    for path in std::iter::once(&paths.credentials_file()).chain(fallbacks) {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| (t().delete_failed)(&path.display().to_string()));
            }
        }
    }
    Ok(())
}

/// 디렉터리를 만들고(없으면) 권한을 0700으로 맞춘다.
pub fn ensure_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| (t().mkdir_failed)(&dir.display().to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, with_lang};
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
        let k = resolve_api_key(Some(" lin_api_env ".into()), &paths, &[])
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_env");
        assert_eq!(k.source, KeySource::Env);
    }

    #[test]
    fn file_key_used_when_env_empty() {
        let (_d, paths) = temp_paths();
        save_api_key(&paths, "  lin_api_file  ").unwrap();
        let k = resolve_api_key(Some("".into()), &paths, &[])
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_file");
        assert_eq!(k.source, KeySource::File);
    }

    #[test]
    fn missing_or_blank_file_means_no_key() {
        let (_d, paths) = temp_paths();
        assert_eq!(resolve_api_key(None, &paths, &[]).unwrap(), None);
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.credentials_file(), "  \n").unwrap();
        assert_eq!(resolve_api_key(None, &paths, &[]).unwrap(), None);
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
        delete_credentials(&paths, &[]).unwrap();
        delete_credentials(&paths, &[]).unwrap();
        assert_eq!(resolve_api_key(None, &paths, &[]).unwrap(), None);
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

    #[test]
    fn out_of_range_values_fall_back() {
        let (_d, paths) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            paths.config_file(),
            "[side]\nrefresh_seconds = 99999\n[cache]\nretention_days = 99999999999\n[agent]\ninclude_comments = -1\n",
        )
        .unwrap();
        let (s, w) = load_settings(&paths.config_file());
        assert_eq!(s, Settings::default());
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(w.iter().any(|m| m.contains("1~3650")), "{w:?}");
    }

    #[test]
    fn fallback_credentials_are_found() {
        let (d, paths) = temp_paths();
        let other = d.path().join("other").join("credentials");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, "lin_api_other\n").unwrap();
        let k = resolve_api_key(None, &paths, std::slice::from_ref(&other))
            .unwrap()
            .unwrap();
        assert_eq!(k.value, "lin_api_other");
        // 주 위치에 키가 있으면 그쪽이 먼저다
        save_api_key(&paths, "lin_api_primary").unwrap();
        let k = resolve_api_key(None, &paths, &[other]).unwrap().unwrap();
        assert_eq!(k.value, "lin_api_primary");
    }

    #[test]
    fn delete_removes_fallback_keys_too() {
        let (d, paths) = temp_paths();
        let other = d.path().join("other").join("credentials");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, "lin_api_other\n").unwrap();
        save_api_key(&paths, "lin_api_primary").unwrap();
        delete_credentials(&paths, std::slice::from_ref(&other)).unwrap();
        assert!(!paths.credentials_file().exists());
        assert!(!other.exists());
    }

    #[test]
    fn fallback_locations_cover_cli_and_plugin() {
        let f = credential_fallbacks(Path::new("/home/me"));
        assert_eq!(
            f,
            vec![
                PathBuf::from("/home/me/.config/herdr-linear/credentials"),
                PathBuf::from("/home/me/.config/herdr/plugins/config/jh.linear/credentials"),
            ]
        );
    }

    #[test]
    fn cache_locations_cover_cli_and_plugin() {
        assert_eq!(
            cache_fallbacks(Path::new("/home/me")),
            vec![
                PathBuf::from("/home/me/.local/state/herdr-linear/cache.db"),
                PathBuf::from("/home/me/.local/state/herdr/plugins/jh.linear/cache.db"),
            ]
        );
    }

    #[test]
    fn reads_language() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "language = \"ja\"\n").unwrap();
        let (s, w) = load_settings(&path);
        assert_eq!(s.language, Lang::Ja);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn language_defaults_to_english() {
        let dir = tempfile::tempdir().unwrap();
        let (s, w) = load_settings(&dir.path().join("config.toml"));
        assert_eq!(s.language, Lang::En);
        assert!(w.is_empty());
    }

    #[test]
    fn unsupported_or_bad_language_warns_and_uses_english() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "language = \"zh-TW\"\n").unwrap();
        let (s, w) = load_settings(&path);
        assert_eq!(s.language, Lang::En);
        assert_eq!(
            w,
            vec![
                "language = \"zh-TW\"는 지원하지 않아요. 영어를 써요 (en, ko, ja, zh-CN, de)"
                    .to_string()
            ]
        );
        std::fs::write(&path, "language = 3\n").unwrap();
        let (s, w) = load_settings(&path);
        assert_eq!(s.language, Lang::En);
        assert_eq!(
            w,
            vec!["language는 문자열이어야 해요. 영어를 써요".to_string()]
        );
        std::fs::write(&path, "language = \"xx\"\n").unwrap();
        let w = with_lang(Lang::En, || load_settings(&path).1);
        assert_eq!(
            w,
            vec![
                "language \"xx\" is not supported. Using English (en, ko, ja, zh-CN, de)"
                    .to_string()
            ]
        );
    }

    #[test]
    fn language_survives_other_bad_keys_but_not_broken_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "language = \"de\"\nteams = 3\n").unwrap();
        let (s, w) = load_settings(&path);
        assert_eq!(s.language, Lang::De);
        assert_eq!(w.len(), 1, "{w:?}");
        std::fs::write(&path, "language = \"de\"\nteams = [\n").unwrap();
        let (s, w) = load_settings(&path);
        assert_eq!(s.language, Lang::En);
        assert_eq!(w.len(), 1, "{w:?}");
    }

    /// `text`를 담은 임시 config.toml을 읽는다.
    fn load_text(text: &str) -> (Settings, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, text).unwrap();
        load_settings(&path)
    }

    #[test]
    fn language_below_a_section_header_is_reported_not_applied() {
        // TOML은 `[side]` 아래의 키를 `side.language`로 읽는다: 말없이 영어로 남지 않게 알린다
        let (s, w) = load_text("[side]\nrefresh_seconds = 30\nlanguage = \"ko\"\n");
        assert_eq!(s.language, Lang::En);
        assert_eq!(s.side_refresh_seconds, 30, "나머지 항목은 그대로 읽는다");
        assert_eq!(
            w,
            vec!["language가 [side] 안에 있어서 쓰지 않았어요. config.toml 맨 위로 옮기세요"]
        );
        // 다른 구역(`[cache]`, `[agent]`)과 읽지 않는 구역(`[display]`)도 그 구역 이름을 알린다
        for section in ["cache", "agent", "display"] {
            let (s, w) = load_text(&format!("[{section}]\nlanguage = \"ko\"\n"));
            assert_eq!(s.language, Lang::En, "{section}");
            assert!(
                w.contains(&format!(
                    "language가 [{section}] 안에 있어서 쓰지 않았어요. config.toml 맨 위로 옮기세요"
                )),
                "{section}: {w:?}"
            );
        }
        // 맨 위에 둔 language는 구역이 뒤따라도 그대로 읽는다
        let (s, w) = load_text("language = \"ko\"\n[side]\nrefresh_seconds = 30\n");
        assert_eq!(s.language, Lang::Ko);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn misspelled_top_level_key_is_reported() {
        let (s, w) = load_text("langauge = \"ko\"\n");
        assert_eq!(s.language, Lang::En);
        assert_eq!(
            w,
            vec!["config.toml의 langauge는 알 수 없는 항목이라 무시해요"]
        );
        // 구역 이름이 틀려도 같다
        let (_, w) = load_text("[sdie]\nrefresh_seconds = 30\n");
        assert_eq!(w, vec!["config.toml의 sdie는 알 수 없는 항목이라 무시해요"]);
    }

    #[test]
    fn unknown_key_inside_a_known_section_is_reported_with_its_section() {
        let (s, w) = load_text("[side]\nrefresh = 30\n");
        assert_eq!(s.side_refresh_seconds, 60, "틀린 이름의 값은 쓰지 않는다");
        assert_eq!(
            w,
            vec!["config.toml의 side.refresh는 알 수 없는 항목이라 무시해요"]
        );
        let (_, w) = load_text("[cache]\nretention = 3\n[agent]\ninclude_comment = 2\n");
        assert_eq!(
            w,
            vec![
                "config.toml의 agent.include_comment는 알 수 없는 항목이라 무시해요",
                "config.toml의 cache.retention는 알 수 없는 항목이라 무시해요",
            ]
        );
    }

    #[test]
    fn a_full_valid_config_has_no_warnings() {
        let (s, w) = load_text(
            "language = \"ja\"\nteams = [\"ENG\"]\n\
             [side]\nrefresh_seconds = 30\n\
             [cache]\nretention_days = 7\n\
             [agent]\ninclude_comments = 2\ntemplate = \"{title}\"\n",
        );
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(s.language, Lang::Ja);
        assert_eq!(s.teams, vec!["ENG"]);
        assert_eq!(s.side_refresh_seconds, 30);
        assert_eq!(s.cache_retention_days, 7);
        assert_eq!(s.agent_include_comments, 2);
        assert_eq!(s.agent_template, "{title}");
    }

    #[test]
    fn key_warnings_come_in_a_fixed_order_after_the_existing_ones() {
        // 값이 틀린 항목의 경고가 먼저, 그다음 키 점검 경고가 키 이름순으로 나온다
        let (_, w) = load_text(
            "teams = 3\nzzz = 1\n[side]\nrefresh_seconds = \"fast\"\nlanguage = \"ko\"\nspeed = 2\n[display]\nlanguage = \"ko\"\n",
        );
        assert_eq!(
            w,
            vec![
                "teams는 문자열 배열이어야 해요. 기본값을 써요",
                "side.refresh_seconds는 0~3600 사이의 정수여야 해요. 기본값을 써요",
                "config.toml의 display는 알 수 없는 항목이라 무시해요",
                "language가 [display] 안에 있어서 쓰지 않았어요. config.toml 맨 위로 옮기세요",
                "language가 [side] 안에 있어서 쓰지 않았어요. config.toml 맨 위로 옮기세요",
                "config.toml의 side.speed는 알 수 없는 항목이라 무시해요",
                "config.toml의 zzz는 알 수 없는 항목이라 무시해요",
            ]
        );
    }

    #[test]
    fn key_warnings_follow_the_language() {
        let w = with_lang(Lang::En, || {
            load_text("[cache]\nlanguage = \"ko\"\nlangauge = \"ko\"\n").1
        });
        assert_eq!(
            w,
            vec![
                "Unknown key cache.langauge in config.toml is ignored",
                "language is inside [cache], so it was ignored. Move it to the top of config.toml",
            ]
        );
        let w = with_lang(Lang::En, || load_text("langauge = \"ko\"\n").1);
        assert_eq!(w, vec!["Unknown key langauge in config.toml is ignored"]);
    }
}
