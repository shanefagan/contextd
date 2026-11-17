//! End-to-end tests: run the real contextd binary on sockets in a temporary
//! directory and talk Varlink to it, including the abuse cases from the
//! v0.9.2 code review.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Daemon {
    child: Child,
    dir: tempfile::TempDir,
}

impl Daemon {
    fn start(rgb: bool, config: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, config).unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_contextd"));
        cmd.arg("--runtime-dir")
            .arg(dir.path())
            .arg("--config")
            .arg(&config_path)
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if rgb {
            cmd.arg("--rgb");
        }
        let child = cmd.spawn().unwrap();
        let d = Self { child, dir };
        let wait_for = if rgb { d.control() } else { d.core() };
        let deadline = Instant::now() + Duration::from_secs(10);
        while UnixStream::connect(&wait_for).is_err() {
            assert!(Instant::now() < deadline, "daemon did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        d
    }

    fn core(&self) -> PathBuf {
        self.dir.path().join("public/contextd.socket")
    }
    fn observer(&self) -> PathBuf {
        self.dir.path().join("public/contextd-rgb-observer.socket")
    }
    fn control(&self) -> PathBuf {
        self.dir.path().join("private/contextd-rgb-control.socket")
    }

    fn rss_kib(&self) -> u64 {
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.child.id())).unwrap();
        status
            .lines()
            .find_map(|l| l.strip_prefix("VmRSS:"))
            .and_then(|v| v.split_whitespace().next()?.parse().ok())
            .unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn connect(path: &Path) -> UnixStream {
    let s = UnixStream::connect(path).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s
}

fn send(s: &mut UnixStream, method: &str, params: Value, more: bool) {
    let mut msg = json!({"method": method, "parameters": params});
    if more {
        msg["more"] = json!(true);
    }
    let mut bytes = serde_json::to_vec(&msg).unwrap();
    bytes.push(0);
    s.write_all(&bytes).unwrap();
}

fn recv(s: &mut UnixStream) -> std::io::Result<Value> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if s.read(&mut byte)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        if byte[0] == 0 {
            return Ok(serde_json::from_slice(&buf).unwrap());
        }
        buf.push(byte[0]);
    }
}

fn call(path: &Path, method: &str, params: Value) -> Value {
    let mut s = connect(path);
    send(&mut s, method, params, false);
    recv(&mut s).unwrap()
}

const CORE: &str = "com.performativenonsense.contextd.";
const OBS: &str = "com.performativenonsense.contextd.rgb.Observer.";
const CTL: &str = "com.performativenonsense.contextd.rgb.Control.";

#[test]
fn broken_config_refuses_to_start() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    std::fs::write(&cfg, "[auth]\nauthorised_units = [\"openrgb.service\"]\n").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_contextd"))
        .arg("--runtime-dir")
        .arg(dir.path())
        .arg("--config")
        .arg(&cfg)
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn core_answers_basic_calls() {
    let d = Daemon::start(false, "");
    let r = call(&d.core(), &format!("{CORE}GetActiveGame"), json!({}));
    assert!(r.get("error").is_none(), "{r}");
    let r = call(&d.core(), &format!("{CORE}ListRunningGames"), json!({}));
    assert!(r["parameters"]["games"].is_array(), "{r}");
    let r = call(&d.core(), &format!("{CORE}ListDevices"), json!({}));
    assert!(r["parameters"]["devices"].is_array(), "{r}");
}

#[test]
fn idle_connections_do_not_block_other_clients() {
    let d = Daemon::start(false, "");
    // v0.9.2 served four connections at a time; the fifth client hung.
    let idle: Vec<_> = (0..12).map(|_| connect(&d.core())).collect();
    let r = call(&d.core(), &format!("{CORE}GetActiveGame"), json!({}));
    assert!(r.get("error").is_none(), "{r}");
    drop(idle);
}

#[test]
fn per_uid_connection_limit_is_enforced() {
    let d = Daemon::start(false, "[server]\nmax_connections_per_uid = 3\n");
    // Let the startup probe connection's slot be released first.
    std::thread::sleep(Duration::from_millis(300));
    let _idle: Vec<_> = (0..3).map(|_| connect(&d.core())).collect();
    std::thread::sleep(Duration::from_millis(100));
    let mut extra = connect(&d.core());
    send(
        &mut extra,
        &format!("{CORE}GetActiveGame"),
        json!({}),
        false,
    );
    assert!(
        recv(&mut extra).is_err(),
        "over-limit connection must be closed"
    );
}

#[test]
fn oversized_message_is_rejected_without_growing_memory() {
    let d = Daemon::start(false, "");
    let before = d.rss_kib();
    let mut s = connect(&d.core());
    let chunk = vec![b'A'; 1 << 20];
    let mut sent = 0;
    for _ in 0..64 {
        if s.write_all(&chunk).is_err() {
            break; // daemon closed the connection, as it should
        }
        sent += 1;
    }
    std::thread::sleep(Duration::from_millis(200));
    let after = d.rss_kib();
    assert!(
        sent < 64 || after < before + 16 * 1024,
        "RSS grew from {before} to {after} KiB"
    );
    let r = call(&d.core(), &format!("{CORE}GetActiveGame"), json!({}));
    assert!(r.get("error").is_none(), "daemon still serves after abuse");
}

#[test]
fn controller_hints_are_bound_to_the_caller() {
    let d = Daemon::start(false, "");
    let hint = |pid: i64, name: &str| json!({"controller": {"name": name, "pid": pid, "capabilities": [], "interested_devices": []}});

    // A process that is not the caller or one of its ancestors: our child.
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let r = call(
        &d.core(),
        &format!("{CORE}RegisterController"),
        hint(other.id() as i64, "Impostor"),
    );
    let _ = other.kill();
    let _ = other.wait();
    assert_eq!(r["error"], format!("{CORE}InvalidController"), "{r}");

    // pid 0 means "me"; keep the connection open so our PID stays the peer.
    let mut s = connect(&d.core());
    send(
        &mut s,
        &format!("{CORE}RegisterController"),
        hint(0, "Self"),
        false,
    );
    assert!(recv(&mut s).unwrap().get("error").is_none());
    let r = call(&d.core(), &format!("{CORE}ListControllers"), json!({}));
    let list = r["parameters"]["controllers"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["pid"], std::process::id() as i64);

    let long = "x".repeat(10_000);
    let r = call(
        &d.core(),
        &format!("{CORE}RegisterController"),
        hint(0, &long),
    );
    assert_eq!(r["error"], format!("{CORE}InvalidController"), "{r}");
}

#[test]
fn control_socket_is_not_world_writable() {
    let d = Daemon::start(true, "");
    let mode = std::fs::metadata(d.control()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o660);
    let mode = std::fs::metadata(d.observer())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o666);
}

#[test]
fn matrix_overflow_is_rejected_and_matrix_can_be_cleared() {
    let d = Daemon::start(true, "");
    let huge = 1i64 << 32;
    let r = call(
        &d.control(),
        &format!("{CTL}SetLightingContext"),
        json!({"matrix": {"width": huge, "height": huge, "data": []}}),
    );
    assert_eq!(r["error"], format!("{CTL}MatrixTooLarge"), "{r}");

    let px = json!({"r": 1, "g": 2, "b": 3, "a": 255});
    call(
        &d.control(),
        &format!("{CTL}SetLightingContext"),
        json!({"matrix": {"width": 1, "height": 1, "data": [px]}}),
    );
    let r = call(
        &d.observer(),
        &format!("{OBS}GetLightingContext"),
        json!({}),
    );
    assert_eq!(r["parameters"]["matrix"]["width"], 1);

    let r = call(
        &d.control(),
        &format!("{CTL}SetLightingContext"),
        json!({"clear_matrix": true}),
    );
    assert!(r.get("error").is_none(), "{r}");
    let r = call(
        &d.observer(),
        &format!("{OBS}GetLightingContext"),
        json!({}),
    );
    assert!(
        r["parameters"].get("matrix").is_none_or(Value::is_null),
        "{r}"
    );
}

#[test]
fn subscribers_get_updates_and_are_reaped_when_they_leave() {
    let d = Daemon::start(true, "[server]\nmax_connections_per_uid = 4\n");

    let mut sub = connect(&d.observer());
    send(
        &mut sub,
        &format!("{OBS}SubscribeLightingContext"),
        json!({}),
        true,
    );
    let first = recv(&mut sub).unwrap();
    assert_eq!(first["continues"], true);
    call(
        &d.control(),
        &format!("{CTL}SetLightingContext"),
        json!({"main_color": {"r": 255, "g": 0, "b": 0, "a": 255}}),
    );
    let update = recv(&mut sub).unwrap();
    assert_eq!(update["parameters"]["main_color"]["r"], 255);
    drop(sub);
    std::thread::sleep(Duration::from_millis(1500));

    // Fill the per-UID limit with subscribers, hang up, and make sure the
    // daemon notices without any lighting update arriving.
    let subs: Vec<_> = (0..4)
        .map(|_| {
            let mut s = connect(&d.observer());
            send(
                &mut s,
                &format!("{OBS}SubscribeLightingContext"),
                json!({}),
                true,
            );
            recv(&mut s).unwrap();
            s
        })
        .collect();
    drop(subs);
    std::thread::sleep(Duration::from_millis(1500));
    let r = call(
        &d.observer(),
        &format!("{OBS}GetLightingContext"),
        json!({}),
    );
    assert!(r.get("error").is_none(), "{r}");
}

#[test]
fn running_games_subscription_sends_initial_state_and_no_repeats() {
    let d = Daemon::start(false, "");
    let mut sub = connect(&d.core());
    send(
        &mut sub,
        &format!("{CORE}SubscribeRunningGames"),
        json!({}),
        true,
    );
    let first = recv(&mut sub).unwrap();
    assert_eq!(first["continues"], true, "{first}");
    assert!(first["parameters"]["games"].is_array());

    // Nothing changes, so nothing more arrives (the read times out).
    sub.set_read_timeout(Some(Duration::from_millis(2500)))
        .unwrap();
    assert!(recv(&mut sub).is_err(), "no update without a change");
}

#[test]
fn subscriptions_without_more_behave_like_list_calls() {
    let d = Daemon::start(false, "");
    for method in ["SubscribeRunningGames", "SubscribeDevices"] {
        let r = call(&d.core(), &format!("{CORE}{method}"), json!({}));
        assert!(r.get("error").is_none(), "{method}: {r}");
        assert!(
            r.get("continues").is_none_or(|c| c == false),
            "{method}: {r}"
        );
    }
}

#[test]
fn device_subscription_starts_and_reaps_cleanly() {
    let d = Daemon::start(false, "[server]\nmax_connections_per_uid = 3\n");
    std::thread::sleep(Duration::from_millis(300));
    for _ in 0..2 {
        let mut subs: Vec<_> = (0..3)
            .map(|_| {
                let mut s = connect(&d.core());
                send(&mut s, &format!("{CORE}SubscribeDevices"), json!({}), true);
                let r = recv(&mut s).unwrap();
                assert!(r["parameters"]["devices"].is_array(), "{r}");
                s
            })
            .collect();
        subs.clear();
        std::thread::sleep(Duration::from_millis(1500));
    }
}

#[test]
fn focus_reports_are_checked() {
    let d = Daemon::start(false, "");
    // Our own process: accepted.
    let mut s = connect(&d.core());
    send(
        &mut s,
        &format!("{CORE}ReportFocus"),
        json!({"pid": std::process::id()}),
        false,
    );
    assert!(recv(&mut s).unwrap().get("error").is_none());
    // A PID that does not exist: rejected.
    let r = call(
        &d.core(),
        &format!("{CORE}ReportFocus"),
        json!({"pid": 999_999_999}),
    );
    assert_eq!(r["error"], format!("{CORE}InvalidFocus"), "{r}");
}
