use std::fmt;
use std::path::Path;
use std::process::Command;

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
