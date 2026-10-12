//! Bounded desktop clipboard access. Linux Wayland uses wl-clipboard; X11 uses xclip.
//! Missing helpers, unavailable displays and timeouts are explicit errors.
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
pub trait ClipboardAccess {
    fn read_text(&mut self) -> Result<String, String>;
    fn write_text(&mut self, text: &str) -> Result<(), String>;
}
#[derive(Default)]
pub struct SystemClipboard;
impl SystemClipboard {
    pub fn new() -> Self {
        Self
    }
}
impl ClipboardAccess for SystemClipboard {
    fn read_text(&mut self) -> Result<String, String> {
        let (program, args) = helper(false)?;
        let data = transfer(program, &args, None)?;
        String::from_utf8(data).map_err(|_| "Clipboard does not contain UTF-8 text".into())
    }
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        if text.len() > MAX_TEXT_BYTES {
            return Err("Clipboard text exceeds 16 MiB".into());
        }
        let (program, args) = helper(true)?;
        transfer(program, &args, Some(text.as_bytes().to_vec())).map(|_| ())
    }
}
fn helper(write: bool) -> Result<(&'static str, Vec<&'static str>), String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        Ok(if write {
            ("wl-copy", vec!["--type", "text/plain;charset=utf-8"])
        } else {
            ("wl-paste", vec!["--no-newline", "--type", "text"])
        })
    } else if std::env::var_os("DISPLAY").is_some() {
        Ok((
            "xclip",
            vec![
                "-selection",
                "clipboard",
                if write { "-in" } else { "-out" },
            ],
        ))
    } else {
        Err("No desktop clipboard display is available".into())
    }
}
fn transfer(program: &str, args: &[&str], data: Option<Vec<u8>>) -> Result<Vec<u8>, String> {
    let writing = data.is_some();
    let mut child = Command::new(program)
        .args(args)
        .stdin(if writing {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(if writing {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Clipboard helper {program}: {e}"))?;
    let input = child.stdin.take();
    let output = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let result = (|| -> Result<Vec<u8>, String> {
            if let (Some(mut pipe), Some(bytes)) = (input, data) {
                pipe.write_all(&bytes).map_err(|e| e.to_string())?;
            }
            let mut bytes = Vec::new();
            if let Some(pipe) = output {
                pipe.take((MAX_TEXT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
            }
            if bytes.len() > MAX_TEXT_BYTES {
                return Err("Clipboard text exceeds 16 MiB".into());
            }
            Ok(bytes)
        })();
        let _ = tx.send(result);
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut result = None;
    loop {
        if result.is_none() {
            result = rx.try_recv().ok();
        }
        if matches!(result, Some(Err(_))) {
            let _ = child.kill();
            let _ = child.wait();
            return result.unwrap();
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!("Clipboard helper {program} failed ({status})"));
                }
                if let Some(value) = result {
                    return value;
                }
            }
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Clipboard operation timed out".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_preserves_exact_newlines_and_reports_failure() {
        assert_eq!(
            transfer("sh", &["-c", "printf 'line1\\nline2\\n'"], None).unwrap(),
            b"line1\nline2\n"
        );
        assert!(transfer("sh", &["-c", "exit 7"], None).is_err());
        assert!(transfer("freefall-absent-clipboard-helper", &[], None).is_err());
    }
    #[test]
    fn helper_write_closes_stdin_and_does_not_interpret_text() {
        transfer(
            "sh",
            &["-c", "cat >/dev/null"],
            Some(b"$(no-execution)\n\"quoted\"\n".to_vec()),
        )
        .unwrap();
    }
}
