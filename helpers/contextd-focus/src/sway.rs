//! Sway (and other i3-IPC compositors): subscribe to window events and read
//! the focused node's PID from the tree.
//!
//! Message framing: `"i3-ipc"`, payload length (u32), message type (u32),
//! payload; integers in native byte order.

use crate::FocusSource;
use anyhow::{Context, bail};
use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

const MAGIC: &[u8; 6] = b"i3-ipc";
const SUBSCRIBE: u32 = 2;
const GET_TREE: u32 = 4;
const EVENT_WINDOW: u32 = 0x8000_0003;

pub struct Sway {
    socket: PathBuf,
    events: UnixStream,
}

fn send(s: &mut UnixStream, kind: u32, payload: &[u8]) -> std::io::Result<()> {
    let mut msg = Vec::with_capacity(14 + payload.len());
    msg.extend_from_slice(MAGIC);
    msg.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
    msg.extend_from_slice(&kind.to_ne_bytes());
    msg.extend_from_slice(payload);
    s.write_all(&msg)
}

fn recv(s: &mut UnixStream) -> anyhow::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 14];
    s.read_exact(&mut header)?;
    if &header[..6] != MAGIC {
        bail!("bad i3-ipc magic");
    }
    let len = u32::from_ne_bytes(header[6..10].try_into()?) as usize;
    let kind = u32::from_ne_bytes(header[10..14].try_into()?);
    if len > 64 * 1024 * 1024 {
        bail!("i3-ipc message too large");
    }
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload)?;
    Ok((kind, payload))
}

/// The PID of the focused node in a `GET_TREE` reply.
pub fn focused_pid(node: &Value) -> Option<u32> {
    if node.get("focused").and_then(Value::as_bool) == Some(true) {
        return node
            .get("pid")
            .and_then(Value::as_i64)
            .filter(|&p| p > 0)
            .and_then(|p| u32::try_from(p).ok());
    }
    ["nodes", "floating_nodes"]
        .iter()
        .filter_map(|k| node.get(*k).and_then(Value::as_array))
        .flatten()
        .find_map(focused_pid)
}

impl Sway {
    pub fn new(socket: PathBuf) -> anyhow::Result<Self> {
        let mut events = UnixStream::connect(&socket)
            .with_context(|| format!("cannot connect to {}", socket.display()))?;
        send(&mut events, SUBSCRIBE, br#"["window"]"#)?;
        let (_, reply) = recv(&mut events)?;
        let ok: Value = serde_json::from_slice(&reply)?;
        if ok.get("success").and_then(Value::as_bool) != Some(true) {
            bail!("compositor refused the window event subscription");
        }
        Ok(Self { socket, events })
    }
}

impl FocusSource for Sway {
    fn current(&mut self) -> anyhow::Result<Option<u32>> {
        let mut s = UnixStream::connect(&self.socket)?;
        send(&mut s, GET_TREE, b"")?;
        let (_, tree) = recv(&mut s)?;
        Ok(focused_pid(&serde_json::from_slice(&tree)?))
    }

    fn wait(&mut self) -> anyhow::Result<()> {
        loop {
            let (kind, _) = recv(&mut self.events)?;
            if kind == EVENT_WINDOW {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::net::UnixListener;

    #[test]
    fn finds_focused_node_anywhere() {
        let tree = json!({"focused": false, "nodes": [
            {"focused": false, "nodes": [{"focused": false, "pid": 1}]},
            {"focused": false, "nodes": [], "floating_nodes": [{"focused": true, "pid": 4242}]}
        ]});
        assert_eq!(focused_pid(&tree), Some(4242));
        assert_eq!(focused_pid(&json!({"focused": false, "nodes": []})), None);
    }

    #[test]
    fn subscribes_and_reads_tree() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sway.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            // Event connection: subscribe, then one window event.
            let (mut ev, _) = listener.accept().unwrap();
            let (kind, payload) = recv(&mut ev).unwrap();
            assert_eq!(
                (kind, payload.as_slice()),
                (SUBSCRIBE, &br#"["window"]"#[..])
            );
            send(&mut ev, SUBSCRIBE, br#"{"success": true}"#).unwrap();
            send(&mut ev, EVENT_WINDOW, br#"{"change": "focus"}"#).unwrap();
            // Tree request connection.
            let (mut req, _) = listener.accept().unwrap();
            let (kind, _) = recv(&mut req).unwrap();
            assert_eq!(kind, GET_TREE);
            send(
                &mut req,
                GET_TREE,
                br#"{"focused": false, "nodes": [{"focused": true, "pid": 99}]}"#,
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(5));
        });
        let mut s = Sway::new(path).unwrap();
        s.wait().unwrap();
        assert_eq!(s.current().unwrap(), Some(99));
    }
}
