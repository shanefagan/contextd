//! Small helpers for reading process information from `/proc`.

use std::fs;

/// Iterates over the PIDs currently listed in `/proc`.
pub fn pids() -> impl Iterator<Item = i32> {
    fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
}

/// Fields from `/proc/<pid>/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcStatus {
    pub ppid: i32,
    /// Effective UID, which is what `SO_PEERCRED` reports.
    pub euid: u32,
}

pub fn status(pid: i32) -> Option<ProcStatus> {
    let content = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    parse_status(&content)
}

fn parse_status(content: &str) -> Option<ProcStatus> {
    let mut ppid = None;
    let mut euid = None;
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("PPid:") {
            ppid = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Uid:") {
            euid = v.split_whitespace().nth(1).and_then(|s| s.parse().ok());
        }
    }
    Some(ProcStatus {
        ppid: ppid?,
        euid: euid?,
    })
}

/// Process start time in clock ticks after boot (field 22 of
/// `/proc/<pid>/stat`). Together with the PID this identifies a process
/// uniquely, which guards against PID reuse.
pub fn start_time(pid: i32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_start_time(&content)
}

fn parse_start_time(stat: &str) -> Option<u64> {
    // The command name (field 2) is in parentheses and may contain spaces
    // or ')' itself, so split after the last ')'. Field 3 is then index 0.
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(22 - 3)?.parse().ok()
}

/// Resident set size in pages (second field of `/proc/<pid>/statm`).
pub fn rss_pages(pid: i32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    content.split_whitespace().nth(1)?.parse().ok()
}

/// First command-line argument of a process.
pub fn argv0(pid: i32) -> Option<String> {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let first = raw.split(|&b| b == 0).next()?;
    (!first.is_empty()).then(|| String::from_utf8_lossy(first).into_owned())
}

/// Reads the selected environment variables of a process. Only keys listed
/// in `keys` are returned, so callers never hold a whole environment (which
/// may contain secrets) in memory longer than this call.
pub fn environ_subset(pid: i32, keys: &[&'static str]) -> Vec<(&'static str, String)> {
    let Ok(raw) = fs::read(format!("/proc/{pid}/environ")) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in raw.split(|&b| b == 0) {
        for key in keys {
            if entry.len() > key.len()
                && entry.starts_with(key.as_bytes())
                && entry[key.len()] == b'='
                && !found.iter().any(|(k, _)| k == key)
            {
                found.push((
                    *key,
                    String::from_utf8_lossy(&entry[key.len() + 1..]).into_owned(),
                ));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status() {
        let s = "Name:\tbash\nPPid:\t42\nUid:\t1000\t1001\t1000\t1000\n";
        assert_eq!(
            parse_status(s),
            Some(ProcStatus {
                ppid: 42,
                euid: 1001
            })
        );
    }

    #[test]
    fn parses_start_time_with_awkward_comm() {
        let mut fields = vec!["0"; 50];
        fields[22 - 3] = "123456";
        let stat = format!("77 (evil ) name) {}", fields.join(" "));
        assert_eq!(parse_start_time(&stat), Some(123456));
    }

    #[test]
    fn reads_own_process() {
        let me = std::process::id() as i32;
        assert!(start_time(me).is_some());
        assert_eq!(status(me).unwrap().euid, unsafe { libc::geteuid() });
    }
}
