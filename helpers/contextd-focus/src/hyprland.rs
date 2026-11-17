//! Hyprland: event socket (`.socket2.sock`) plus request socket
//! (`.socket.sock`) for `j/activewindow`.

use crate::FocusSource;
use anyhow::{Context, bail};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

pub struct Hyprland {
    dir: PathBuf,
    events: BufReader<UnixStream>,
}

impl Hyprland {
    pub fn from_env() -> anyhow::Result<Self> {
        let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")?;
        let mut candidates = Vec::new();
        if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
            candidates.push(PathBuf::from(runtime).join("hypr").join(&sig));
        }
        candidates.push(PathBuf::from("/tmp/hypr").join(&sig));
        let Some(dir) = candidates
            .into_iter()
            .find(|d| d.join(".socket2.sock").exists())
        else {
            bail!("Hyprland sockets for instance {sig} not found");
        };
        Self::new(dir)
    }

    pub fn new(dir: PathBuf) -> anyhow::Result<Self> {
        let events = UnixStream::connect(dir.join(".socket2.sock"))
            .with_context(|| format!("cannot connect to {}", dir.display()))?;
        Ok(Self {
            dir,
            events: BufReader::new(events),
        })
    }
}

/// Sends one request to Hyprland's request socket.
fn request(dir: &Path, command: &str) -> anyhow::Result<String> {
    let mut s = UnixStream::connect(dir.join(".socket.sock"))?;
    s.write_all(command.as_bytes())?;
    s.shutdown(std::net::Shutdown::Write)?;
    let mut out = String::new();
    s.read_to_string(&mut out)?;
    Ok(out)
}

/// The PID in a `j/activewindow` reply (`{}` when nothing is focused).
pub fn parse_active_window(json: &str) -> Option<u32> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("pid")?
        .as_i64()
        .filter(|&p| p > 0)
        .and_then(|p| u32::try_from(p).ok())
}

impl FocusSource for Hyprland {
    fn current(&mut self) -> anyhow::Result<Option<u32>> {
        Ok(parse_active_window(&request(&self.dir, "j/activewindow")?))
    }

    fn wait(&mut self) -> anyhow::Result<()> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.events.read_line(&mut line)? == 0 {
                bail!("Hyprland closed the event socket");
            }
            if line.starts_with("activewindow>>")
                || line.starts_with("activewindowv2>>")
                || line.starts_with("closewindow>>")
            {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn parses_replies() {
        assert_eq!(
            parse_active_window(r#"{"address":"0x1","pid":4242,"class":"steam_app_1"}"#),
            Some(4242)
        );
        assert_eq!(parse_active_window("{}"), None);
        assert_eq!(parse_active_window(r#"{"pid":-1}"#), None);
    }

    #[test]
    fn follows_events_and_queries_pid() {
        let dir = tempfile::tempdir().unwrap();
        let events = UnixListener::bind(dir.path().join(".socket2.sock")).unwrap();
        let requests = UnixListener::bind(dir.path().join(".socket.sock")).unwrap();
        std::thread::spawn(move || {
            let (mut s, _) = events.accept().unwrap();
            s.write_all(b"workspace>>2\nactivewindowv2>>55aa\n")
                .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(5));
        });
        std::thread::spawn(move || {
            let (mut s, _) = requests.accept().unwrap();
            let mut req = String::new();
            s.read_to_string(&mut req).unwrap();
            assert_eq!(req, "j/activewindow");
            s.write_all(br#"{"pid": 777}"#).unwrap();
        });
        let mut h = Hyprland::new(dir.path().to_path_buf()).unwrap();
        h.wait().unwrap();
        assert_eq!(h.current().unwrap(), Some(777));
    }
}
