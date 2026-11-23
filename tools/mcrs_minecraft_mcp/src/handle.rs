use std::fs::DirBuilder;
use std::fs::File;
use std::io::Write;
use std::net::TcpListener;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Server,
    Vanilla,
    Client,
}

impl Kind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Server => "s",
            Self::Vanilla => "v",
            Self::Client => "c",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Handle {
    pub id: String,
    pub kind: Kind,
    pub pid: u32,
    pub port: Option<u16>,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub workdir: PathBuf,
    #[serde(default)]
    pub control: Option<PathBuf>,
}

pub struct RunDir {
    pub id: String,
}

pub struct Launch<'a> {
    pub kind: Kind,
    pub program: PathBuf,
    pub workdir: &'a Path,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub port: Option<u16>,
    pub control: Option<PathBuf>,
    pub run: &'a RunDir,
}

pub fn state_dir() -> PathBuf {
    std::env::temp_dir().join("mcrs_minecraft_mcp")
}

pub fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    Ok(port)
}

impl RunDir {
    pub fn create(kind: Kind) -> Result<Self, String> {
        let state = state_dir();
        std::fs::create_dir_all(&state).map_err(|error| error.to_string())?;
        for number in 1..100_000 {
            let id = format!("{}{number}", kind.prefix());
            if DirBuilder::new()
                .mode(0o700)
                .create(state.join(&id))
                .is_ok()
            {
                return Ok(Self { id });
            }
        }
        Err(format!("no free run directory in {}", state.display()))
    }

    pub fn path(&self) -> PathBuf {
        state_dir().join(&self.id)
    }
}

impl Handle {
    pub fn dir(&self) -> PathBuf {
        state_dir().join(&self.id)
    }

    pub fn log(&self) -> PathBuf {
        self.dir().join("log.txt")
    }

    pub fn load(id: &str) -> Result<Self, String> {
        let path = state_dir().join(id).join("handle.json");
        let text = std::fs::read_to_string(&path).map_err(|_| format!("no handle {id}"))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn all() -> Vec<Self> {
        let Ok(entries) = std::fs::read_dir(state_dir()) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| Self::load(&entry.ok()?.file_name().into_string().ok()?).ok())
            .collect()
    }

    pub fn age_s(&self) -> Option<u64> {
        let modified = std::fs::metadata(self.dir().join("handle.json"))
            .ok()?
            .modified();
        Some(modified.ok()?.elapsed().ok()?.as_secs())
    }

    pub fn forget(&self) {
        std::fs::remove_file(self.dir().join("handle.json")).ok();
    }

    pub fn alive(&self) -> bool {
        signal(self.pid, 0)
    }

    pub fn send(&self, line: &str) -> Result<(), String> {
        if !self.alive() {
            return Err(format!("{} is not running", self.id));
        }
        let mut stdin = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(self.dir().join("stdin"))
            .map_err(|error| format!("{} takes no input: {error}", self.id))?;
        writeln!(stdin, "{line}").map_err(|error| error.to_string())
    }

    pub fn stop(&self) -> &'static str {
        if let Some(control) = &self.control {
            std::fs::remove_file(control).ok();
        }
        self.forget();
        if !self.alive() {
            return "had exited";
        }
        signal_group(self.pid, libc::SIGTERM);
        if wait_until_dead(self, Duration::from_secs(10)) {
            return "stopped";
        }
        signal_group(self.pid, libc::SIGKILL);
        wait_until_dead(self, Duration::from_secs(2));
        "killed"
    }
}

fn wait_until_dead(handle: &Handle, timeout: Duration) -> bool {
    let start = Instant::now();
    while handle.alive() {
        if start.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

fn signal(pid: u32, signal: i32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: kill takes plain integers and touches no memory of this process.
    unsafe { libc::kill(pid, signal) == 0 }
}

fn signal_group(pid: u32, number: i32) {
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: kill takes plain integers and touches no memory of this process.
        unsafe { libc::kill(-pid, number) };
    }
}

pub struct Supervisor {
    pub workdir: PathBuf,
    session: String,
    watchdog: Option<std::process::ChildStdin>,
}

const WATCHDOG: &str = "trap '' INT TERM HUP; while read run; do set -- \"$@\" \"$run\"; done; \
for run; do [ -e \"${run#* }/handle.json\" ] && kill -TERM -\"${run%% *}\"; done 2>/dev/null; sleep 5; \
for run; do [ -e \"${run#* }/handle.json\" ] && kill -KILL -\"${run%% *}\"; done 2>/dev/null";

impl Supervisor {
    pub fn command_line(workdir: PathBuf, session: Option<String>) -> Self {
        Self {
            session: session.unwrap_or_else(|| workdir.display().to_string()),
            workdir,
            watchdog: None,
        }
    }

    pub fn serving(workdir: PathBuf) -> Result<Self, String> {
        let watchdog = Command::new("/bin/sh")
            .args(["-c", WATCHDOG])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|error| format!("the watchdog did not start: {error}"))?;
        Ok(Self {
            workdir,
            session: format!("mcp-{}", std::process::id()),
            watchdog: watchdog.stdin,
        })
    }

    pub fn owns(&self, handle: &Handle) -> bool {
        handle.session == self.session
    }

    pub fn spawn(&mut self, launch: Launch<'_>) -> Result<Handle, String> {
        let dir = launch.run.path();
        let stdin = dir.join("stdin");
        let made = Command::new("mkfifo").arg(&stdin).status();
        if !made.is_ok_and(|status| status.success()) {
            return Err(format!("mkfifo {} failed", stdin.display()));
        }
        let open = |path: &Path| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|error| format!("{}: {error}", path.display()))
        };
        let input = open(&stdin)?;
        let log = File::create(dir.join("log.txt")).map_err(|error| error.to_string())?;
        let log_err = log.try_clone().map_err(|error| error.to_string())?;
        let mut child = Command::new(&launch.program)
            .args(&launch.args)
            .envs(launch.env.iter().map(|(key, value)| (key, value)))
            .current_dir(&dir)
            .stdin(input)
            .stdout(log)
            .stderr(log_err)
            .process_group(0)
            .spawn()
            .map_err(|error| format!("{}: {error}", launch.program.display()))?;
        let handle = Handle {
            id: launch.run.id.clone(),
            kind: launch.kind,
            pid: child.id(),
            port: launch.port,
            session: self.session.clone(),
            workdir: launch.workdir.to_owned(),
            control: launch.control,
        };
        if let Some(watchdog) = &mut self.watchdog {
            writeln!(watchdog, "{} {}", handle.pid, dir.display())
                .map_err(|error| error.to_string())?;
            std::thread::spawn(move || child.wait());
        }
        let text = serde_json::to_string(&handle).map_err(|error| error.to_string())?;
        std::fs::write(dir.join("handle.json"), text).map_err(|error| error.to_string())?;
        Ok(handle)
    }
}
