//! 로그 파일. 팔레트가 화면을 차지하는 동안 오류는 여기에 남긴다. 키는 남기지 않는다.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// 이 크기를 넘으면 `.1`로 하나 백업하고 새로 쓴다.
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

pub struct Logger {
    path: PathBuf,
}

impl Logger {
    pub fn new(path: PathBuf) -> Logger {
        Logger { path }
    }

    /// 한 줄 남긴다. 실패해도 조용히 넘어간다.
    pub fn write(&self, msg: &str) {
        if fs::metadata(&self.path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
            let mut backup = self.path.clone().into_os_string();
            backup.push(".1");
            let _ = fs::rename(&self.path, PathBuf::from(backup));
        }
        let line = format!(
            "{} {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            crate::markdown::sanitize(msg).replace('\n', " ")
        );
        let open = || {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        };
        // 처음 실행이면 상태 디렉터리가 아직 없을 수 있다 (캐시를 열기 전에 실패한 경우)
        let file = open().or_else(|e| match self.path.parent() {
            Some(dir) if e.kind() == std::io::ErrorKind::NotFound => {
                crate::config::ensure_private_dir(dir).map_err(std::io::Error::other)?;
                open()
            }
            _ => Err(e),
        });
        if let Ok(mut f) = file {
            let _ = f.write_all(line.as_bytes());
        }
    }

    /// 실패면 `what: 오류`로 한 줄 남기고 결과를 그대로 돌려준다.
    pub fn on_err<T>(&self, what: &str, r: anyhow::Result<T>) -> anyhow::Result<T> {
        r.inspect_err(|e| self.write(&format!("{what}: {e:#}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_lines_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("herdr-linear.log");
        let log = Logger::new(path.clone());
        log.write("첫 줄\n이어짐\u{1b}[2J");
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.ends_with("첫 줄 이어짐[2J\n"), "{text:?}");
        fs::write(&path, vec![b'x'; (MAX_LOG_BYTES + 1) as usize]).unwrap();
        log.write("새 파일");
        assert!(fs::read_to_string(&path).unwrap().ends_with("새 파일\n"));
        assert!(dir.path().join("herdr-linear.log.1").exists());
    }

    #[test]
    fn creates_the_state_dir_and_logs_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("herdr-linear.log");
        let log = Logger::new(path.clone());
        let r: anyhow::Result<()> = Err(anyhow::anyhow!("키 파일을 읽지 못했어요"));
        assert!(log.on_err("팔레트 시작", r).is_err());
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.ends_with("팔레트 시작: 키 파일을 읽지 못했어요\n"),
            "{text:?}"
        );
    }
}
