//! herdr CLI 호출: 플러그인 pane 열기와 알림.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::config::PLUGIN_ID;
use crate::context::Origin;

pub struct Herdr {
    bin: PathBuf,
}

impl Herdr {
    /// herdr가 액션에 넘겨 준 `HERDR_BIN_PATH`, 없으면 PATH의 `herdr`.
    pub fn from_env() -> Herdr {
        let bin = std::env::var_os("HERDR_BIN_PATH")
            .filter(|v| !v.is_empty())
            .map_or_else(|| PathBuf::from("herdr"), PathBuf::from);
        Herdr::new(bin)
    }

    pub fn new(bin: PathBuf) -> Herdr {
        Herdr { bin }
    }

    /// 이 플러그인의 pane을 띄우고 포커스를 옮긴다. `env`는 pane 프로세스의 환경 변수가 된다.
    pub fn open_pane(&self, entrypoint: &str, env: &[(String, String)]) -> Result<()> {
        let mut args: Vec<String> = [
            "plugin",
            "pane",
            "open",
            "--plugin",
            PLUGIN_ID,
            "--entrypoint",
            entrypoint,
            "--focus",
        ]
        .map(String::from)
        .to_vec();
        for (k, v) in env {
            args.push("--env".into());
            args.push(format!("{k}={v}"));
        }
        self.run(&args)
    }

    /// herdr 알림. 액션은 출력이 보이지 않아서 결과를 이걸로 알린다.
    pub fn notify(&self, title: &str, body: &str) -> Result<()> {
        self.run(&[
            "notification".into(),
            "show".into(),
            title.into(),
            "--body".into(),
            body.into(),
        ])
    }

    fn run(&self, args: &[String]) -> Result<()> {
        let out = Command::new(&self.bin)
            .args(args)
            .output()
            .with_context(|| format!("{}을 실행하지 못했어요", self.bin.display()))?;
        if out.status.success() {
            return Ok(());
        }
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.contains("ui_busy") {
            bail!("herdr에 설정·복사 모드 같은 다른 창이 떠 있어요. 닫고 다시 시도하세요");
        }
        if err.is_empty() {
            bail!("herdr 명령이 실패했어요 ({})", out.status);
        }
        bail!("herdr 명령이 실패했어요: {err}")
    }
}

/// `open palette`·`open url` 액션: 원래 pane의 맥락을 팔레트 pane에 넘겨 띄운다.
/// 실패하면 액션 출력은 보이지 않으니 herdr 알림으로도 알린다.
pub fn open_palette(herdr: &Herdr, origin: &Origin) -> Result<()> {
    herdr
        .open_pane("palette", &origin.to_env())
        .inspect_err(|e| {
            let _ = herdr.notify("Linear", &format!("팔레트를 열지 못했어요: {e:#}"));
        })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::context::PluginContext;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    /// 인자를 한 줄씩 `args.log`에 남기는 가짜 herdr. `plugin` 명령은 `fail_plugin`이면 실패한다.
    fn fake_herdr(dir: &Path, fail_plugin: Option<&str>) -> PathBuf {
        let bin = dir.join("herdr");
        let log = dir.join("args.log");
        let fail = match fail_plugin {
            Some(err) => format!("if [ \"$1\" = plugin ]; then echo '{err}' >&2; exit 1; fi\n"),
            None => String::new(),
        };
        let script = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{log}'; done\necho '--' >> '{log}'\n{fail}exit 0\n",
            log = log.display()
        );
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    fn calls(dir: &Path) -> Vec<Vec<String>> {
        std::fs::read_to_string(dir.join("args.log"))
            .unwrap_or_default()
            .split("--\n")
            .filter(|c| !c.is_empty())
            .map(|c| c.lines().map(String::from).collect())
            .collect()
    }

    #[test]
    fn opens_palette_with_origin_env() {
        let dir = tempfile::tempdir().unwrap();
        let herdr = Herdr::new(fake_herdr(dir.path(), None));
        let ctx = PluginContext::parse(
            r#"{"focused_pane_id":"w1:p2","focused_pane_cwd":"/repo app","selected_text":"eng-7"}"#,
        );
        open_palette(&herdr, &Origin::from_context(&ctx)).unwrap();
        assert_eq!(
            calls(dir.path()),
            vec![vec![
                "plugin",
                "pane",
                "open",
                "--plugin",
                "jh.linear",
                "--entrypoint",
                "palette",
                "--focus",
                "--env",
                "HERDR_LINEAR_ORIGIN_PANE=w1:p2",
                "--env",
                "HERDR_LINEAR_ORIGIN_CWD=/repo app",
                "--env",
                "HERDR_LINEAR_OPEN=ENG-7",
            ]]
        );
    }

    #[test]
    fn busy_herdr_is_reported_by_notification() {
        let dir = tempfile::tempdir().unwrap();
        let herdr = Herdr::new(fake_herdr(dir.path(), Some("error: ui_busy")));
        let err = open_palette(&herdr, &Origin::default()).unwrap_err();
        assert!(err.to_string().contains("다른 창이 떠 있어요"), "{err}");
        let calls = calls(dir.path());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1][..4], ["notification", "show", "Linear", "--body"]);
        assert!(calls[1][4].starts_with("팔레트를 열지 못했어요"));
    }

    #[test]
    fn missing_herdr_binary_is_an_error() {
        let herdr = Herdr::new(PathBuf::from("/nonexistent/herdr"));
        assert!(herdr.notify("Linear", "x").is_err());
    }
}
