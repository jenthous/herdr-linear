//! 실제 바이너리로 언어 설정을 확인한다: 설정이 없으면 영어, `language`로 바꾼다.
//! 키·herdr 환경 변수를 비운 임시 HOME에서 돌려서 네트워크에 닿지 않는다.

use std::path::Path;
use std::process::{Command, Output};

/// 임시 HOME으로 `herdr-linear`를 돌린다. `LINEAR_API_KEY`와 `HERDR_*`는 비운다.
fn run(home: &Path, extra_env: &[(&str, &Path)], args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_herdr-linear"));
    cmd.args(args)
        .env("HOME", home)
        .env_remove("LINEAR_API_KEY");
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("HERDR_") {
            cmd.env_remove(&k);
        }
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.output().expect("herdr-linear runs")
}

/// 단독 CLI의 설정 파일을 쓴다.
fn write_cli_config(home: &Path, text: &str) {
    let dir = home.join(".config").join("herdr-linear");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), text).unwrap();
}

fn has_hangul(s: &str) -> bool {
    s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn no_config_is_english() {
    let home = tempfile::tempdir().unwrap();
    let out = run(home.path(), &[], &["whoami"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("error: No API key."), "{err}");
    assert!(!has_hangul(&err), "{err}");
}

#[test]
fn language_ko_is_korean() {
    let home = tempfile::tempdir().unwrap();
    write_cli_config(home.path(), "language = \"ko\"\n");
    let err = stderr(&run(home.path(), &[], &["whoami"]));
    assert!(err.contains("오류: API 키가 없어요."), "{err}");
}

#[test]
fn unknown_language_is_english_with_a_warning() {
    let home = tempfile::tempdir().unwrap();
    write_cli_config(home.path(), "language = \"xx\"\n");
    let err = stderr(&run(home.path(), &[], &["whoami"]));
    assert!(
        err.contains("warning: language \"xx\" is not supported. Using English"),
        "{err}"
    );
    assert!(err.contains("error: No API key."), "{err}");
}

#[test]
fn language_below_a_section_header_is_reported_and_not_applied() {
    // TOML이 `[side]` 아래의 `language`를 `side.language`로 읽는 실수: 영어로 남되 말없이 넘어가지 않는다
    let home = tempfile::tempdir().unwrap();
    write_cli_config(
        home.path(),
        "[side]\nrefresh_seconds = 30\nlanguage = \"ko\"\n",
    );
    let err = stderr(&run(home.path(), &[], &["whoami"]));
    assert!(
        err.contains(
            "warning: language is inside [side], so it was ignored. Move it to the top of config.toml"
        ),
        "{err}"
    );
    assert!(err.contains("error: No API key."), "{err}");
    assert!(!has_hangul(&err), "{err}");
}

#[test]
fn misspelled_key_is_reported() {
    let home = tempfile::tempdir().unwrap();
    write_cli_config(home.path(), "langauge = \"ko\"\n");
    let err = stderr(&run(home.path(), &[], &["whoami"]));
    assert!(
        err.contains("warning: Unknown key langauge in config.toml is ignored"),
        "{err}"
    );
}

#[test]
fn plugin_config_dir_sets_the_language() {
    let home = tempfile::tempdir().unwrap();
    let plugin = home.path().join("plugin-config");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("config.toml"), "language = \"ko\"\n").unwrap();
    let err = stderr(&run(
        home.path(),
        &[("HERDR_PLUGIN_CONFIG_DIR", &plugin)],
        &["whoami"],
    ));
    assert!(err.contains("API 키가 없어요"), "{err}");
}

#[test]
fn help_follows_the_language() {
    let home = tempfile::tempdir().unwrap();
    let out = run(home.path(), &[], &["--help"]);
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("Fast Linear lookup in herdr"), "{help}");
    assert!(!has_hangul(&help), "{help}");
    for sub in [
        "login", "logout", "whoami", "mine", "search", "show", "open", "ui",
    ] {
        let out = run(home.path(), &[], &[sub, "--help"]);
        assert!(out.status.success(), "{sub}");
        let help = String::from_utf8_lossy(&out.stdout);
        assert!(!has_hangul(&help), "{sub}: {help}");
    }
    write_cli_config(home.path(), "language = \"ko\"\n");
    let help = String::from_utf8_lossy(&run(home.path(), &[], &["--help"]).stdout).into_owned();
    assert!(help.contains("herdr에서 Linear를 빠르게 조회"), "{help}");
}
