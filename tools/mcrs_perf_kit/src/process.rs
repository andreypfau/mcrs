use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Instant;

use anyhow::Context;

/// A command for a measured binary that drops the tester's own `MCRS_*` variables, so only the
/// knobs the kit sets shape a run, and every variable that is not UTF-8, on which build scripts
/// that read the whole environment panic (zsh exports `_` metafied under a non-ASCII path).
#[expect(
    clippy::disallowed_methods,
    reason = "the kit reads the tester's environment only to keep it out of the measured binaries"
)]
pub fn clean_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    for (key, value) in std::env::vars_os() {
        let kept = match (key.to_str(), value.to_str()) {
            (Some(name), Some(_)) => !name.to_ascii_uppercase().starts_with("MCRS_"),
            _ => false,
        };
        if !kept {
            command.env_remove(key);
        }
    }
    command
}

pub struct Line {
    pub at: Instant,
    pub text: String,
}

pub enum Next {
    Line(Line),
    Timeout,
    Exited,
}

/// A child whose standard output and error arrive as timed lines and are copied to a raw log.
pub struct Process {
    child: Child,
    incoming: Receiver<Line>,
    raw: BufWriter<File>,
    pub started: Instant,
}

impl Process {
    pub fn spawn(mut command: Command, raw_log: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = raw_log.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = BufWriter::new(File::create(raw_log)?);
        let started = Instant::now();
        let mut child = command
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {}", command.get_program().to_string_lossy()))?;
        let (send, incoming) = channel();
        if let Some(stdout) = child.stdout.take() {
            let send = send.clone();
            std::thread::spawn(move || read_lines(stdout, &send));
        }
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || read_lines(stderr, &send));
        }
        Ok(Self {
            child,
            incoming,
            raw,
            started,
        })
    }

    /// The next line before `deadline`; `Exited` once both streams are closed.
    pub fn next_line(&mut self, deadline: Instant) -> Next {
        let wait = deadline.saturating_duration_since(Instant::now());
        match self.incoming.recv_timeout(wait) {
            Ok(line) => {
                let _ = writeln!(self.raw, "{}", line.text);
                Next::Line(line)
            }
            Err(RecvTimeoutError::Timeout) => Next::Timeout,
            Err(RecvTimeoutError::Disconnected) => Next::Exited,
        }
    }

    pub fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        while let Ok(line) = self.incoming.try_recv() {
            let _ = writeln!(self.raw, "{}", line.text);
        }
        let _ = self.raw.flush();
    }
}

fn read_lines(output: impl Read, send: &Sender<Line>) {
    let mut reader = BufReader::new(output);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&bytes);
        let line = Line {
            at: Instant::now(),
            text: text.trim_end_matches(['\r', '\n']).to_owned(),
        };
        if send.send(line).is_err() {
            return;
        }
    }
}
