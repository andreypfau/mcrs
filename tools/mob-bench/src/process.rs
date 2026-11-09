//! The server process: its console on standard input and its log, one timed line at a time.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};

use crate::probe;

pub struct Line {
    pub at: Instant,
    pub text: String,
}

pub struct ServerProcess {
    child: Child,
    console: ChildStdin,
    incoming: Receiver<Line>,
    pub log: Vec<Line>,
    /// The answers to `damage` hits so far. They are counted, not kept in the log: the stir gets
    /// one a pig every 80 ticks, millions in a run.
    pub damage_answers: u64,
    raw: BufWriter<File>,
}

/// How the server is asked to stop.
#[derive(Clone, Copy, Debug)]
pub enum Stop {
    Console,
    Interrupt,
}

impl ServerProcess {
    /// Starts `command` in `dir` with standard error joined to standard output, and copies every
    /// line to `log_file`.
    pub fn start(
        command: &[String],
        env: &[(String, String)],
        dir: &Path,
        log_file: &Path,
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
        Ok(Self {
            child,
            console,
            incoming,
            log: Vec::new(),
            damage_answers: 0,
            raw: BufWriter::new(File::create(log_file)?),
        })
    }

    pub fn pid(&self) -> i32 {
        self.child.id() as i32
    }

    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    pub fn send(&mut self, line: &str) -> anyhow::Result<()> {
        writeln!(self.console, "{line}")?;
        Ok(self.console.flush()?)
    }

    /// Moves the lines that arrived into [`Self::log`].
    pub fn pump(&mut self) {
        while let Ok(line) = self.incoming.try_recv() {
            if probe::damage_applied(&line.text) {
                self.damage_answers = self.damage_answers.saturating_add(1);
                continue;
            }
            let _ = writeln!(self.raw, "{}", line.text);
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

    /// Asks the server to stop, kills it after `grace`, and flushes the log file.
    pub fn stop(mut self, stop: Stop, grace: Duration) {
        match stop {
            Stop::Console => {
                let _ = self.send("stop");
            }
            Stop::Interrupt => {
                let _ = Command::new("kill")
                    .arg("-INT")
                    .arg(self.pid().to_string())
                    .status();
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
