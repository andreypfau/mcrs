use std::fmt;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug)]
pub enum Failure {
    Spawn {
        command: String,
        reason: String,
    },
    Exit {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Spawn { command, reason } => write!(f, "could not run {command}: {reason}"),
            Failure::Exit {
                command, stderr, ..
            } => write!(f, "{command} failed: {stderr}"),
        }
    }
}

pub fn run(repo: &Path, args: &[&str]) -> Result<Vec<u8>, Failure> {
    let command = format!("git {}", args.join(" "));
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| Failure::Spawn {
            command: command.clone(),
            reason: e.to_string(),
        })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(Failure::Exit {
        command,
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

pub struct Output {
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

pub fn output(repo: &Path, args: &[&str]) -> Result<Output, Failure> {
    let command = format!("git {}", args.join(" "));
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| Failure::Spawn {
            command,
            reason: e.to_string(),
        })?;
    Ok(Output {
        status: output.status.code(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

pub fn pipe(repo: &Path, args: &[&str], input: &[u8]) -> Result<Vec<u8>, Failure> {
    let command = format!("git {}", args.join(" "));
    let spawn_failure = |e: std::io::Error| Failure::Spawn {
        command: command.clone(),
        reason: e.to_string(),
    };
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_failure)?;
    let mut stdin = child.stdin.take().expect("stdin was requested as a pipe");
    let output = std::thread::scope(|scope| {
        scope.spawn(move || {
            let _ = stdin.write_all(input);
        });
        child.wait_with_output()
    })
    .map_err(spawn_failure)?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(Failure::Exit {
        command,
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

pub fn text(repo: &Path, args: &[&str]) -> Result<String, Failure> {
    run(repo, args).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

pub fn differs(repo: &Path, diff_args: &[&str]) -> Result<bool, Failure> {
    let mut args = vec!["diff", "--quiet"];
    args.extend_from_slice(diff_args);
    match run(repo, &args) {
        Ok(_) => Ok(false),
        Err(Failure::Exit { code: Some(1), .. }) => Ok(true),
        Err(other) => Err(other),
    }
}
