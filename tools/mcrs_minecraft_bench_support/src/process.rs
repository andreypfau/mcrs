//! The server process: its console on standard input and its log, one timed line at a time.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};

pub struct Line {
    pub at: Instant,
    pub text: String,
}

/// How the server is asked to stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Console,
    Interrupt,
}

pub struct ServerProcess {
    child: Child,
    console: ChildStdin,
    incoming: Receiver<Line>,
    log: Vec<Line>,
    counted: Option<fn(&str) -> bool>,
    counted_lines: u64,
    raw: Option<BufWriter<File>>,
}

impl ServerProcess {
    /// Starts `command` in `dir` with standard error joined to standard output, and copies every
    /// kept line to `log_file` when there is one.
    pub fn start(
        command: &[String],
        env: &[(String, String)],
        dir: &Path,
        log_file: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let Some((program, arguments)) = command.split_first() else {
            bail!("no server command");
        };
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(r#"exec "$@" 2>&1"#)
            .arg("sh")
            .arg(program)
            .args(arguments)
            .envs(env.iter().map(|(key, value)| (key, value)))
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {program}"))?;
        let (Some(console), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            bail!("the server has no console");
        };
        let (send, incoming) = channel();
        std::thread::spawn(move || read_lines(output, &send));
        let raw = log_file
            .map(|path| File::create(path).map(BufWriter::new))
            .transpose()?;
        Ok(Self {
            child,
            console,
            incoming,
            log: Vec::new(),
            counted: None,
            counted_lines: 0,
            raw,
        })
    }

    /// Counts the lines `matches` accepts instead of keeping them: a stir answers one line a
    /// hit, millions in a run.
    #[must_use]
    pub fn counting(mut self, matches: fn(&str) -> bool) -> Self {
        self.counted = Some(matches);
        self
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    pub fn send(&mut self, line: &str) -> anyhow::Result<()> {
        writeln!(self.console, "{line}")?;
        Ok(self.console.flush()?)
    }

    pub fn log(&self) -> &[Line] {
        &self.log
    }

    pub fn counted_lines(&self) -> u64 {
        self.counted_lines
    }

    /// Moves the lines that arrived into the log.
    pub fn pump(&mut self) {
        while let Ok(line) = self.incoming.try_recv() {
            if self.counted.is_some_and(|matches| matches(&line.text)) {
                self.counted_lines = self.counted_lines.saturating_add(1);
                continue;
            }
            if let Some(raw) = &mut self.raw {
                let _ = writeln!(raw, "{}", line.text);
            }
            self.log.push(line);
        }
    }

    /// Waits up to `timeout` for a line that arrives at `after` or later and that `parse`
    /// accepts, and returns its value.
    pub fn wait_for<T>(
        &mut self,
        after: Instant,
        timeout: Duration,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        let deadline = Instant::now() + timeout;
        let mut checked = self.log.partition_point(|line| line.at < after);
        loop {
            self.pump();
            while let Some(line) = self.log.get(checked) {
                checked += 1;
                if let Some(value) = parse(&line.text) {
                    return Some(value);
                }
            }
            if Instant::now() >= deadline || self.exited() {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The lines that arrived in `from..until`.
    pub fn lines_between(&self, from: Instant, until: Instant) -> &[Line] {
        let start = self.log.partition_point(|line| line.at < from);
        let end = self.log.partition_point(|line| line.at < until);
        self.log.get(start..end).unwrap_or_default()
    }

    /// Asks the server to stop, kills it after `grace`, flushes the log file and returns the log.
    pub fn stop(mut self, stop: Stop, grace: Duration) -> Vec<Line> {
        match stop {
            Stop::Console => {
                let _ = self.send("stop");
            }
            #[cfg(unix)]
            Stop::Interrupt => {
                let _ = Command::new("kill")
                    .arg("-INT")
                    .arg(self.pid().to_string())
                    .status();
            }
            // chisle: Windows has no SIGINT for a child, so the server is killed at once and
            // saves nothing; a console `stop` command in mcrs lifts this.
            #[cfg(not(unix))]
            Stop::Interrupt => {
                let _ = self.child.kill();
            }
        }
        let deadline = Instant::now() + grace;
        while !self.exited() && Instant::now() < deadline {
            self.pump();
            std::thread::sleep(Duration::from_millis(200));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        std::thread::sleep(Duration::from_millis(200));
        self.pump();
        if let Some(raw) = &mut self.raw {
            let _ = raw.flush();
        }
        self.log
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
