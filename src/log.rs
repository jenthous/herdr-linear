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
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = f.write_all(line.as_bytes());
        }
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
}
