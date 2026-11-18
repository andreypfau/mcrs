use anyhow::Context;
use anyhow::bail;
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::BuildHasher;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdin;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const START_LIMIT: Duration = Duration::from_secs(300);
const CONSOLE_LIMIT: Duration = Duration::from_secs(120);
const REPLY_PREFIX: &str = "System chat: ";

pub(crate) struct VanillaServer {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
    nonce: u64,
    marker: u32,
}

impl VanillaServer {
    pub(crate) fn start(jar: &Path, directory: &Path, port: u16) -> anyhow::Result<Self> {
        fs::write(directory.join("eula.txt"), "eula=true\n")?;
        let properties = format!(
            "online-mode=false\nserver-ip=127.0.0.1\nserver-port={port}\n\
             level-type=minecraft\\:flat\ngenerate-structures=false\nspawn-protection=0\n\
             view-distance=2\nsimulation-distance=2\nenforce-secure-profile=false\n\
             enable-rcon=false\nenable-query=false\nwhite-list=false\n"
        );
        fs::write(directory.join("server.properties"), properties)?;
        let mut child = Command::new("java")
            .arg("-Xmx2G")
            .arg("-jar")
            .arg(jar)
            .arg("nogui")
            .current_dir(directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("could not start java")?;
        let input = child.stdin.take().context("no server stdin")?;
        let stdout = child.stdout.take().context("no server stdout")?;
        let (sender, output) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        let server = Self {
            child,
            input,
            output,
            nonce: RandomState::new().hash_one(std::process::id()),
            marker: 0,
        };
        server.wait_for("Done (", START_LIMIT)?;
        Ok(server)
    }

    pub(crate) fn echo(&self) {
        for line in self.output.try_iter() {
            println!("server: {line}");
        }
    }

    pub(crate) fn console(&mut self, lines: &[String]) -> anyhow::Result<Vec<String>> {
        self.console_before(lines, Instant::now() + CONSOLE_LIMIT)
    }

    pub(crate) fn console_before(
        &mut self,
        lines: &[String],
        deadline: Instant,
    ) -> anyhow::Result<Vec<String>> {
        anyhow::ensure!(
            Instant::now() < deadline,
            "console deadline already expired"
        );
        self.marker += 1;
        let start_marker = format!("mcrs-probe-{:016x}-start-{}", self.nonce, self.marker);
        let end_marker = format!("mcrs-probe-{:016x}-end-{}", self.nonce, self.marker);
        writeln!(self.input, "say {start_marker}")?;
        for line in lines {
            writeln!(self.input, "{line}")?;
        }
        writeln!(self.input, "say {end_marker}")?;
        self.input.flush()?;
        let mut started = false;
        let mut replies = Vec::new();
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = match self.output.recv_timeout(remaining) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    bail!("the server ended before it printed `{end_marker}`")
                }
            };
            if Instant::now() >= deadline {
                break;
            }
            println!("server: {line}");
            if !started {
                if matches_say_marker(&line, &start_marker) {
                    started = true;
                }
                continue;
            }
            if matches_say_marker(&line, &end_marker) {
                return Ok(replies);
            }
            if let Some((_, reply)) = line.split_once(REPLY_PREFIX) {
                replies.push(reply.to_owned());
            }
        }
        bail!("the server did not print `{end_marker}` before the console deadline")
    }

    pub(crate) fn stop(mut self) -> anyhow::Result<()> {
        self.shut_down()
    }

    pub(crate) fn restart(
        &mut self,
        jar: &Path,
        directory: &Path,
        port: u16,
    ) -> anyhow::Result<()> {
        self.shut_down()?;
        self.echo();
        *self = Self::start(jar, directory, port)?;
        Ok(())
    }

    fn shut_down(&mut self) -> anyhow::Result<()> {
        writeln!(self.input, "stop")?;
        self.input.flush()?;
        self.child.wait()?;
        Ok(())
    }

    fn wait_for(&self, text: &str, limit: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            let line = match self.output.recv_timeout(Duration::from_secs(1)) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    bail!("the server ended before it printed `{text}`")
                }
            };
            println!("server: {line}");
            if line.contains(text) {
                return Ok(());
            }
        }
        bail!("the server did not print `{text}` in {limit:?}")
    }
}

fn matches_say_marker(line: &str, marker: &str) -> bool {
    let Some((_, body)) = line.split_once("]: ") else {
        return false;
    };
    body.strip_prefix("[Not Secure] ")
        .unwrap_or(body)
        .strip_prefix("[Server] ")
        == Some(marker)
}

impl Drop for VanillaServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::matches_say_marker;

    #[test]
    fn matches_say_marker_requires_exact_server_payload() {
        assert!(matches_say_marker(
            "[14:00:00] [Server thread/INFO]: [Not Secure] [Server] token-1",
            "token-1"
        ));
        assert!(matches_say_marker(
            "[14:00:00] [Server thread/INFO]: [Server] token-1",
            "token-1"
        ));
        assert!(!matches_say_marker(
            "[14:00:00] [Server thread/INFO]: [Not Secure] [Server] checkpoint token-1",
            "token-1"
        ));
        assert!(!matches_say_marker(
            "[14:00:00] [Server thread/INFO]: System chat: token-1",
            "token-1"
        ));
    }
}
