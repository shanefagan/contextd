pub use crate::contextd::Game;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub mod bottles;
pub mod heroic;
pub mod itch;
pub mod lutris;
pub mod manager;
pub mod steam;
pub mod vdf;

/// Environment variables read from one process, limited to the keys the
/// detectors asked for.
pub type ProcessEnv = HashMap<&'static str, String>;

/// What contextd knows about one running process.
#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
    pub env: ProcessEnv,
    /// Target of `/proc/<pid>/exe`
    pub exe: Option<PathBuf>,
    /// Target of `/proc/<pid>/cwd`
    pub cwd: Option<PathBuf>,
    /// First command-line argument (for Wine processes, the Windows path
    /// of the program, e.g. `C:\Games\Foo\foo.exe`)
    pub argv0: Option<String>,
}

/// How a running process refers to a game.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameRef {
    /// Matches `Game.id`
    Id(String),
    /// Matches `Game.name`
    Name(String),
}

/// How to recognise an installed game's processes when the launcher does
/// not tag them with an environment variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchHint {
    /// Recognised through `GameDetector::identify`.
    None,
    /// A native executable inside this directory, or a Windows program
    /// (`*.exe`) running with this directory as its working directory.
    InstallDir(PathBuf),
    /// A Wine program: `WINEPREFIX` is `prefix` and the program's file name
    /// is `exe` (compared case-insensitively).
    WineProgram { prefix: PathBuf, exe: String },
}

/// An installed game and how to spot it running.
#[derive(Debug, Clone)]
pub struct Installed {
    pub game: Game,
    pub hint: MatchHint,
}

impl Installed {
    pub fn plain(game: Game) -> Self {
        Self {
            game,
            hint: MatchHint::None,
        }
    }

    /// Whether `process` belongs to this game according to its hint.
    pub fn matches(&self, process: &ProcessInfo) -> bool {
        match &self.hint {
            MatchHint::None => false,
            MatchHint::InstallDir(dir) => {
                let native = process.exe.as_ref().is_some_and(|e| e.starts_with(dir));
                let windows = process.cwd.as_ref().is_some_and(|c| c.starts_with(dir))
                    && process
                        .argv0
                        .as_deref()
                        .is_some_and(|a| a.to_ascii_lowercase().ends_with(".exe"));
                native || windows
            }
            MatchHint::WineProgram { prefix, exe } => {
                let in_prefix = process
                    .env
                    .get("WINEPREFIX")
                    .is_some_and(|p| Path::new(p.trim_end_matches('/')) == prefix.as_path());
                in_prefix
                    && process
                        .argv0
                        .as_deref()
                        .map(windows_file_name)
                        .is_some_and(|name| name.eq_ignore_ascii_case(exe))
            }
        }
    }
}

/// The file name of a Windows or Unix path.
pub fn windows_file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

pub trait GameDetector: Send + Sync {
    /// Unique name of the detector (e.g., "Steam", "Lutris")
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Returns all games installed through this launcher.
    fn list_installed(&self) -> Vec<Installed>;

    /// Environment variables this launcher sets on the processes it starts.
    fn env_keys(&self) -> &'static [&'static str] {
        &[]
    }

    /// Identifies the game a process belongs to from its environment.
    /// `env` contains only the variables from `env_keys` the process has.
    fn identify(&self, _env: &ProcessEnv) -> Option<GameRef> {
        None
    }
}

/// Home directories to scan for launcher data: everything in `/home` plus
/// the homes of regular users from `/etc/passwd` (for homes elsewhere).
pub fn home_dirs() -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = fs::read_dir("/home")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if let Ok(passwd) = fs::read_to_string("/etc/passwd") {
        for home in passwd_homes(&passwd) {
            if home.is_dir() && !homes.contains(&home) {
                homes.push(home);
            }
        }
    }
    homes
}

/// Home directories of regular (UID 1000-65533) accounts.
fn passwd_homes(passwd: &str) -> Vec<PathBuf> {
    passwd
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            let uid: u32 = fields.get(2)?.parse().ok()?;
            let home = fields.get(5)?;
            ((1000..65534).contains(&uid) && home.starts_with('/')).then(|| PathBuf::from(home))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game {
            name: "G".into(),
            id: Some("1".into()),
            source: "T".into(),
            pid: None,
            focused: None,
        }
    }

    #[test]
    fn regular_users_from_passwd() {
        let p = "root:x:0:0::/root:/bin/bash\nalice:x:1000:1000::/data/alice:/bin/fish\nnobody:x:65534:65534::/:/usr/bin/nologin\nsvc:x:990:990::/var/lib/svc:/usr/bin/nologin\n";
        assert_eq!(passwd_homes(p), vec![PathBuf::from("/data/alice")]);
    }

    #[test]
    fn install_dir_matches_native_and_wine_but_not_a_shell() {
        let inst = Installed {
            game: game(),
            hint: MatchHint::InstallDir("/games/Foo".into()),
        };
        let native = ProcessInfo {
            exe: Some("/games/Foo/foo.x86_64".into()),
            ..Default::default()
        };
        let wine = ProcessInfo {
            exe: Some("/usr/bin/wine64-preloader".into()),
            cwd: Some("/games/Foo/bin".into()),
            argv0: Some("Z:\\games\\Foo\\bin\\Foo.exe".into()),
            ..Default::default()
        };
        let shell = ProcessInfo {
            exe: Some("/usr/bin/fish".into()),
            cwd: Some("/games/Foo".into()),
            argv0: Some("fish".into()),
            ..Default::default()
        };
        assert!(inst.matches(&native));
        assert!(inst.matches(&wine));
        assert!(!inst.matches(&shell));
    }

    #[test]
    fn wine_program_matches_prefix_and_exe() {
        let inst = Installed {
            game: game(),
            hint: MatchHint::WineProgram {
                prefix: "/home/a/.local/share/bottles/bottles/Gaming".into(),
                exe: "Game.exe".into(),
            },
        };
        let mut p = ProcessInfo {
            argv0: Some("C:\\Program Files\\Game\\GAME.EXE".into()),
            ..Default::default()
        };
        assert!(!inst.matches(&p), "needs the prefix");
        p.env.insert(
            "WINEPREFIX",
            "/home/a/.local/share/bottles/bottles/Gaming/".into(),
        );
        assert!(inst.matches(&p));
        p.argv0 = Some("C:\\windows\\system32\\services.exe".into());
        assert!(
            !inst.matches(&p),
            "other programs in the bottle are not the game"
        );
    }
}
