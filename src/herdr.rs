//! herdr CLI 호출: 플러그인 pane 열기·닫기, pane 조회, 알림.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::config::{APP_NAME, PLUGIN_ID};
use crate::context::{Origin, PluginContext};
use crate::i18n::t;
use crate::log::Logger;
use crate::side::SidePanes;

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
        self.open_with(entrypoint, &[], env)
    }

    /// 사이드 pane을 지금 pane 오른쪽에 띄운다. 폭은 herdr 기본 분할이다.
    pub fn open_side_pane(&self, env: &[(String, String)]) -> Result<()> {
        self.open_with(
            "side",
            &["--placement", "split", "--direction", "right"],
            env,
        )
    }

    fn open_with(
        &self,
        entrypoint: &str,
        placement: &[&str],
        env: &[(String, String)],
    ) -> Result<()> {
        let mut args: Vec<String> = [
            "plugin",
            "pane",
            "open",
            "--plugin",
            PLUGIN_ID,
            "--entrypoint",
            entrypoint,
        ]
        .map(String::from)
        .to_vec();
        args.extend(placement.iter().map(|a| a.to_string()));
        args.push("--focus".into());
        for (k, v) in env {
            args.push("--env".into());
            args.push(format!("{k}={v}"));
        }
        self.run(&args)
    }

    /// pane이 속한 워크스페이스 id. 없는 pane이면 herdr가 실패한다.
    pub fn pane_workspace(&self, pane: &str) -> Result<String> {
        let out = self.output(&["pane", "get", pane])?;
        json_text(&out, "/result/pane/workspace_id")
    }

    /// 이 명령이 도는 pane의 (pane id, 워크스페이스 id).
    pub fn current_pane(&self) -> Result<(String, String)> {
        let out = self.output(&["pane", "current"])?;
        Ok((
            json_text(&out, "/result/pane/pane_id")?,
            json_text(&out, "/result/pane/workspace_id")?,
        ))
    }

    /// 그 pane에서 우리 사이드 화면(`herdr-linear ui --mode side`)이 도는지. 알 수 없으면 아니라고 본다.
    pub fn runs_side_ui(&self, pane: &str) -> bool {
        let Ok(out) = self.output(&["pane", "process-info", "--pane", pane]) else {
            return false;
        };
        let Ok(v) = serde_json::from_str::<Value>(&out) else {
            return false;
        };
        let processes = v
            .pointer("/result/process_info/foreground_processes")
            .and_then(Value::as_array);
        processes.into_iter().flatten().any(|p| {
            let argv: Vec<&str> = p
                .get("argv")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            is_side_ui(&argv)
        })
    }

    /// 플러그인 pane을 닫는다. 플러그인 pane이 아니거나 없으면 herdr가 거절한다.
    pub fn close_plugin_pane(&self, pane: &str) -> Result<()> {
        self.run(&["plugin", "pane", "close", pane])
    }

    /// herdr 알림. 액션은 출력이 보이지 않아서 결과를 이걸로 알린다.
    pub fn notify(&self, title: &str, body: &str) -> Result<()> {
        self.run(&["notification", "show", title, "--body", body])
    }

    fn run<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<()> {
        self.output(args).map(|_| ())
    }

    /// 명령을 실행하고 표준 출력을 돌려준다.
    fn output<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<String> {
        let out = Command::new(&self.bin)
            .args(args)
            .output()
            .with_context(|| (t().run_failed)(&self.bin.display().to_string()))?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        // 오류는 표준 에러로 온다. 비었으면 표준 출력의 오류 JSON을 쓴다
        let mut err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            err = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
        if err.contains("ui_busy") {
            bail!("{}", t().herdr_busy);
        }
        if err.is_empty() {
            bail!("{}", (t().herdr_failed_status)(&out.status.to_string()));
        }
        bail!("{}", (t().herdr_failed)(&err))
    }
}

/// `open palette`·`open url` 액션: 원래 pane의 맥락을 팔레트 pane에 넘겨 띄운다.
/// 실패하면 액션 출력은 보이지 않으니 로그에 남기고 herdr 알림으로도 알린다.
pub fn open_palette(herdr: &Herdr, origin: &Origin, log: &Logger) -> Result<()> {
    herdr
        .open_pane("palette", &origin.to_env())
        .inspect_err(|e| {
            let text = (t().palette_open_failed)(&format!("{e:#}"));
            log.write(&text);
            let _ = herdr.notify("Linear", &text);
        })
}

/// `open side` 액션: 이 워크스페이스에 우리 사이드 pane이 떠 있으면 닫고, 없으면 지금 pane 오른쪽에 연다.
/// `workspace`는 herdr가 준 `HERDR_WORKSPACE_ID`다. 실패하면 로그에 남기고 herdr 알림으로도 알린다.
pub fn toggle_side(
    herdr: &Herdr,
    panes: &SidePanes,
    workspace: Option<String>,
    ctx: &PluginContext,
    log: &Logger,
) -> Result<()> {
    switch_side(herdr, panes, workspace, ctx).inspect_err(|e| {
        let text = (t().side_toggle_failed)(&format!("{e:#}"));
        log.write(&text);
        let _ = herdr.notify("Linear", &text);
    })
}

fn switch_side(
    herdr: &Herdr,
    panes: &SidePanes,
    workspace: Option<String>,
    ctx: &PluginContext,
) -> Result<()> {
    // 워크스페이스: 환경 변수 → 컨텍스트 JSON → 포커스된 pane을 herdr에 묻기
    let known = [workspace, ctx.workspace_id.clone()]
        .into_iter()
        .flatten()
        .find(|w| !w.trim().is_empty());
    let workspace = match known {
        Some(w) => w,
        None => {
            let pane = ctx
                .focused_pane_id
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .context(t().unknown_workspace)?;
            herdr.pane_workspace(pane)?
        }
    };
    if let Some(pane) = panes.get(&workspace) {
        // pane id는 다시 쓰일 수 있어서, 우리 사이드 화면이 도는 것을 본 뒤에만 닫는다
        if herdr.runs_side_ui(&pane) {
            herdr.close_plugin_pane(&pane)?;
            return panes.remove(&workspace, &pane);
        }
        // 이미 닫혔거나 다른 pane이 됐다. 그 pane은 두고 기록만 지운다
        panes.remove(&workspace, &pane)?;
    }
    herdr.open_side_pane(&Origin::from_context(ctx).to_env())
}

/// 로그아웃: 기록된 사이드 pane 중 우리 사이드 화면이 도는 것만 닫고 기록을 지운다.
/// 열린 사이드 pane이 메모리에 남은 키로 계속 새로고침하지 않게 한다. 닫지 못한 pane의 기록은 남긴다.
pub fn close_side_panes(herdr: &Herdr, panes: &SidePanes) {
    for (workspace, pane) in panes.all() {
        if herdr.runs_side_ui(&pane) && herdr.close_plugin_pane(&pane).is_err() {
            continue;
        }
        let _ = panes.remove(&workspace, &pane);
    }
}

/// `…/herdr-linear ui --mode side`인지.
fn is_side_ui(argv: &[&str]) -> bool {
    let bin = argv
        .first()
        .and_then(|a| Path::new(a).file_name())
        .and_then(|n| n.to_str());
    bin == Some(APP_NAME) && argv.windows(3).any(|w| w == ["ui", "--mode", "side"])
}

/// herdr JSON 출력에서 `pointer` 자리의 글자.
fn json_text(out: &str, pointer: &str) -> Result<String> {
    serde_json::from_str::<Value>(out)
        .ok()
        .and_then(|v| v.pointer(pointer)?.as_str().map(String::from))
        .with_context(|| (t().herdr_missing_field)(pointer))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

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
        let log = Logger::new(dir.path().join("herdr-linear.log"));
        open_palette(&herdr, &Origin::from_context(&ctx), &log).unwrap();
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
        let log_path = dir.path().join("herdr-linear.log");
        let err =
            open_palette(&herdr, &Origin::default(), &Logger::new(log_path.clone())).unwrap_err();
        assert!(err.to_string().contains("다른 창이 떠 있어요"), "{err}");
        let logged = std::fs::read_to_string(&log_path).unwrap();
        assert!(logged.contains("팔레트를 열지 못했어요"), "{logged}");
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

    /// 인자를 `args.log`에 남기는 가짜 herdr. 인자 전체가 `(앞부분, 출력, 종료 코드)`의 앞부분으로
    /// 시작하면 그 출력을 표준 출력에 내고 그 코드로 끝난다. 맞는 것이 없으면 아무것도 내지 않고 성공한다.
    fn scripted_herdr(dir: &Path, replies: &[(&str, &str, i32)]) -> PathBuf {
        let bin = dir.join("herdr");
        let log = dir.join("args.log");
        let mut cases = String::new();
        for (i, (prefix, out, code)) in replies.iter().enumerate() {
            let file = dir.join(format!("reply-{i}"));
            std::fs::write(&file, out).unwrap();
            cases.push_str(&format!(
                "  '{prefix}'*) cat '{}'; exit {code} ;;\n",
                file.display()
            ));
        }
        let script = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{log}'; done\necho '--' >> '{log}'\ncase \"$*\" in\n{cases}esac\nexit 0\n",
            log = log.display()
        );
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    /// `pane process-info`: 우리 사이드 화면이 돈다.
    const SIDE_PROCESS: &str = r#"{"id":"cli:pane:process-info","result":{"process_info":{"foreground_processes":[{"pid":4242,"argv":["/plugins/jh.linear/target/release/herdr-linear","ui","--mode","side"]}]}}}"#;
    /// `pane process-info`: 다른 프로그램(셸)이 돈다.
    const SHELL_PROCESS: &str = r#"{"id":"cli:pane:process-info","result":{"process_info":{"foreground_processes":[{"pid":4243,"argv":["-zsh"]}]}}}"#;
    const PANE_NOT_FOUND: &str =
        r#"{"error":{"code":"pane_not_found","message":"pane not found"}}"#;
    const PANE_IN_W1: &str =
        r#"{"id":"cli:pane:get","result":{"pane":{"pane_id":"w1:p2","workspace_id":"w1"}}}"#;

    struct Side {
        dir: tempfile::TempDir,
        herdr: Herdr,
        panes: SidePanes,
        log: Logger,
    }

    fn side(replies: &[(&str, &str, i32)]) -> Side {
        let dir = tempfile::tempdir().unwrap();
        let herdr = Herdr::new(scripted_herdr(dir.path(), replies));
        let panes = SidePanes::new(&dir.path().join("state"));
        let log = Logger::new(dir.path().join("herdr-linear.log"));
        Side {
            dir,
            herdr,
            panes,
            log,
        }
    }

    fn focused() -> PluginContext {
        PluginContext::parse(r#"{"focused_pane_id":"w1:p2","focused_pane_cwd":"/repo"}"#)
    }

    #[test]
    fn side_opens_right_of_the_pane_when_none_is_recorded() {
        let s = side(&[]);
        toggle_side(&s.herdr, &s.panes, Some("w1".into()), &focused(), &s.log).unwrap();
        assert_eq!(
            calls(s.dir.path()),
            vec![vec![
                "plugin",
                "pane",
                "open",
                "--plugin",
                "jh.linear",
                "--entrypoint",
                "side",
                "--placement",
                "split",
                "--direction",
                "right",
                "--focus",
                "--env",
                "HERDR_LINEAR_ORIGIN_PANE=w1:p2",
                "--env",
                "HERDR_LINEAR_ORIGIN_CWD=/repo",
            ]]
        );
        assert_eq!(
            s.panes.get("w1"),
            None,
            "기록은 사이드 프로세스가 스스로 한다"
        );
    }

    #[test]
    fn side_closes_our_own_pane_and_forgets_it() {
        let s = side(&[("pane process-info --pane w1:p5", SIDE_PROCESS, 0)]);
        s.panes.set("w1", "w1:p5").unwrap();
        toggle_side(&s.herdr, &s.panes, Some("w1".into()), &focused(), &s.log).unwrap();
        assert_eq!(
            calls(s.dir.path()),
            vec![
                vec!["pane", "process-info", "--pane", "w1:p5"],
                vec!["plugin", "pane", "close", "w1:p5"],
            ]
        );
        assert_eq!(s.panes.get("w1"), None);
    }

    #[test]
    fn side_never_closes_a_pane_running_something_else() {
        // id가 셸 pane에 다시 쓰였거나, 그 pane이 이미 닫혔거나, herdr 답을 읽을 수 없다
        for (reply, code) in [
            (SHELL_PROCESS, 0),
            (PANE_NOT_FOUND, 1),
            ("not json", 0),
            ("", 0),
        ] {
            let s = side(&[("pane process-info --pane w1:p5", reply, code)]);
            s.panes.set("w1", "w1:p5").unwrap();
            toggle_side(&s.herdr, &s.panes, Some("w1".into()), &focused(), &s.log).unwrap();
            let calls = calls(s.dir.path());
            assert!(
                !calls.iter().any(|c| c.iter().any(|a| a == "close")),
                "{calls:?}"
            );
            assert_eq!(calls.len(), 2, "{calls:?}");
            assert_eq!(calls[1][..3], ["plugin", "pane", "open"]);
            assert_eq!(s.panes.get("w1"), None, "남은 기록은 지운다");
        }
    }

    #[test]
    fn side_finds_the_workspace_from_the_context_or_the_focused_pane() {
        // 컨텍스트에 워크스페이스 id가 있으면 herdr에 묻지 않는다
        let s = side(&[("pane process-info --pane w1:p5", SIDE_PROCESS, 0)]);
        s.panes.set("w1", "w1:p5").unwrap();
        let ctx = PluginContext::parse(r#"{"workspace_id":"w1","focused_pane_id":"w1:p2"}"#);
        toggle_side(&s.herdr, &s.panes, None, &ctx, &s.log).unwrap();
        assert_eq!(
            calls(s.dir.path())[0],
            ["pane", "process-info", "--pane", "w1:p5"]
        );
        // 그것도 없으면 포커스된 pane이 속한 워크스페이스
        let s = side(&[
            ("pane get w1:p2", PANE_IN_W1, 0),
            ("pane process-info --pane w1:p5", SIDE_PROCESS, 0),
        ]);
        s.panes.set("w1", "w1:p5").unwrap();
        toggle_side(&s.herdr, &s.panes, None, &focused(), &s.log).unwrap();
        assert_eq!(
            calls(s.dir.path()),
            vec![
                vec!["pane", "get", "w1:p2"],
                vec!["pane", "process-info", "--pane", "w1:p5"],
                vec!["plugin", "pane", "close", "w1:p5"],
            ]
        );
    }

    #[test]
    fn empty_workspace_env_falls_back_to_the_context() {
        let s = side(&[("pane process-info --pane w1:p5", SIDE_PROCESS, 0)]);
        s.panes.set("w1", "w1:p5").unwrap();
        let ctx = PluginContext::parse(r#"{"workspace_id":"w1"}"#);
        toggle_side(&s.herdr, &s.panes, Some(" ".into()), &ctx, &s.log).unwrap();
        assert_eq!(s.panes.get("w1"), None, "w1의 사이드 pane을 닫았다");
    }

    #[test]
    fn side_failure_is_logged_and_notified() {
        // 워크스페이스를 알 수 없다
        let s = side(&[]);
        let err =
            toggle_side(&s.herdr, &s.panes, None, &PluginContext::default(), &s.log).unwrap_err();
        assert!(err.to_string().contains("워크스페이스"), "{err}");
        let logged = std::fs::read_to_string(s.dir.path().join("herdr-linear.log")).unwrap();
        assert!(
            logged.contains("사이드 pane을 열거나 닫지 못했어요"),
            "{logged}"
        );
        let calls = calls(s.dir.path());
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0][..4], ["notification", "show", "Linear", "--body"]);
        // 포커스된 pane을 herdr가 모른다
        let s = side(&[("pane get w1:p2", PANE_NOT_FOUND, 1)]);
        let err = toggle_side(&s.herdr, &s.panes, None, &focused(), &s.log).unwrap_err();
        assert!(err.to_string().contains("pane_not_found"), "{err}");
    }

    #[test]
    fn current_pane_reads_both_ids() {
        let s = side(&[(
            "pane current",
            r#"{"id":"cli:pane:current","result":{"pane":{"pane_id":"w2:p7","workspace_id":"w2"}}}"#,
            0,
        )]);
        assert_eq!(
            s.herdr.current_pane().unwrap(),
            ("w2:p7".to_string(), "w2".to_string())
        );
        let broken = side(&[("pane current", "not json", 0)]);
        assert!(broken.herdr.current_pane().is_err());
    }

    #[test]
    fn side_ui_is_recognized_by_its_command() {
        assert!(is_side_ui(&[
            "/p/jh.linear/target/release/herdr-linear",
            "ui",
            "--mode",
            "side"
        ]));
        assert!(is_side_ui(&[
            "./target/release/herdr-linear",
            "ui",
            "--mode",
            "side"
        ]));
        assert!(
            !is_side_ui(&["/p/herdr-linear", "ui", "--mode", "palette"]),
            "팝업"
        );
        assert!(!is_side_ui(&["/usr/bin/vim", "ui", "--mode", "side"]));
        assert!(!is_side_ui(&[]));
    }

    #[test]
    fn logout_closes_only_our_side_panes() {
        let s = side(&[
            ("pane process-info --pane w1:p5", SIDE_PROCESS, 0),
            ("pane process-info --pane w2:p1", SHELL_PROCESS, 0),
        ]);
        s.panes.set("w1", "w1:p5").unwrap();
        s.panes.set("w2", "w2:p1").unwrap();
        close_side_panes(&s.herdr, &s.panes);
        let calls = calls(s.dir.path());
        assert!(
            calls
                .iter()
                .any(|c| *c == ["plugin", "pane", "close", "w1:p5"]),
            "{calls:?}"
        );
        assert!(
            !calls
                .iter()
                .any(|c| c.iter().any(|a| a == "close") && c.iter().any(|a| a == "w2:p1")),
            "셸 pane은 닫지 않는다: {calls:?}"
        );
        assert!(s.panes.all().is_empty(), "남은 기록도 지운다");
    }

    #[test]
    fn logout_keeps_the_record_when_close_fails() {
        let s = side(&[
            ("pane process-info --pane w1:p5", SIDE_PROCESS, 0),
            ("plugin pane close w1:p5", "error: ui_busy", 1),
        ]);
        s.panes.set("w1", "w1:p5").unwrap();
        close_side_panes(&s.herdr, &s.panes);
        assert_eq!(s.panes.get("w1").as_deref(), Some("w1:p5"));
    }
}
