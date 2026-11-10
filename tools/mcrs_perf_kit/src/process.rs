use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Instant;

use anyhow::Context;

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
    for text in BufReader::new(output).lines().map_while(Result::ok) {
        let line = Line {
            at: Instant::now(),
            text,
        };
        if send.send(line).is_err() {
            return;
        }
    }
}
