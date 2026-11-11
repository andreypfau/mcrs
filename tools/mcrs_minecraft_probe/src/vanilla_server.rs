use anyhow::Context;
use anyhow::bail;
use std::fs;
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

/// The vanilla dedicated server as a child process, driven through its console.
pub(crate) struct VanillaServer {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
}

impl VanillaServer {
    /// Starts `jar` in `directory` in offline mode on a new flat world and waits until it is up.
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
        };
        server.wait_for("Done (", START_LIMIT)?;
        Ok(server)
    }

    /// Prints what the server printed since the last call.
    pub(crate) fn echo(&self) {
        for line in self.output.try_iter() {
            println!("server: {line}");
        }
    }

    pub(crate) fn stop(mut self) -> anyhow::Result<()> {
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

/// A probe that fails leaves no server behind on the port.
impl Drop for VanillaServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
