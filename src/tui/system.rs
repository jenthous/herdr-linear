//! 브라우저 열기와 클립보드 복사. 테스트에서 바꿔 끼울 수 있게 trait으로 둔다.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

pub trait System {
    fn open_url(&self, url: &str) -> Result<()>;
    fn copy(&self, text: &str) -> Result<()>;
}

/// 실제 운영체제 명령을 쓴다.
pub struct RealSystem;

impl System for RealSystem {
    fn open_url(&self, url: &str) -> Result<()> {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let mut child = Command::new(opener)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("{opener}을 실행하지 못했어요"))?;
        // 기다리지 않되, 끝나면 거둬서 좀비 프로세스를 남기지 않는다
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    fn copy(&self, text: &str) -> Result<()> {
        // 원격 세션에서도 바깥 터미널 클립보드에 닿도록 OSC 52를 먼저 보낸다 (지원하지 않으면 무시된다)
        let mut out = std::io::stdout();
        let _ = write!(out, "\u{1b}]52;c;{}\u{7}", base64(text.as_bytes()));
        let _ = out.flush();
        for (cmd, args) in clipboard_commands() {
            let Ok(mut child) = Command::new(cmd)
                .args(*args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                continue;
            };
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            if child.wait()?.success() {
                return Ok(());
            }
        }
        Ok(())
    }
}

fn clipboard_commands() -> &'static [(&'static str, &'static [&'static str])] {
    if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["-b", "-i"]),
        ]
    }
}

/// 표준 base64 (OSC 52용).
pub fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(chunk[0]) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let sym = |shift: u32| TABLE[((n >> shift) & 63) as usize] as char;
        out.push(sym(18));
        out.push(sym(12));
        out.push(if chunk.len() > 1 { sym(6) } else { '=' });
        out.push(if chunk.len() > 2 { sym(0) } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64("한".as_bytes()), "7ZWc");
    }
}
