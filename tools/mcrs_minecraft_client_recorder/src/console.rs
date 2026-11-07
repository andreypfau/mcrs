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
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::bail;

const LEVEL: &str = "world-recorder";

/// The vanilla dedicated server, driven through its console.
pub(crate) struct Console {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
}

impl Console {
    /// Starts the server in `directory` on a new flat world and waits until it is up.
    pub(crate) fn start(
        directory: &Path,
        port: u16,
        properties: &[String],
    ) -> anyhow::Result<Self> {
        fs::write(directory.join("eula.txt"), "eula=true\n")?;
        let mut text = format!(
            "online-mode=false\nserver-ip=127.0.0.1\nserver-port={port}\nlevel-name={LEVEL}\n\
             level-type=minecraft\\:flat\ngenerate-structures=false\nspawn-protection=0\n\
             view-distance=6\nsimulation-distance=6\nenable-rcon=false\nenable-query=false\n"
        );
        for property in properties {
            text.push_str(property);
            text.push('\n');
        }
        fs::write(directory.join("server.properties"), text)?;
        let level = directory.join(LEVEL);
        if level.exists() {
            fs::remove_dir_all(level)?;
        }
        let mut child = Command::new("java")
            .args(["-Xmx2G", "-jar", "server.jar", "nogui"])
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
        let console = Self {
            child,
            input,
            output,
        };
        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            let Ok(line) = console.output.recv_timeout(Duration::from_secs(1)) else {
                continue;
            };
            println!("server: {line}");
            if line.contains("Done (") {
                return Ok(console);
            }
        }
        bail!("the server did not start in 300 s")
    }

    pub(crate) fn run(&mut self, command: &str) -> anyhow::Result<()> {
        writeln!(self.input, "{command}")?;
        self.input.flush()?;
        Ok(())
    }

    /// Prints what the server printed since the last call.
    pub(crate) fn echo(&self) {
        for line in self.output.try_iter() {
            println!("server: {line}");
        }
    }

    pub(crate) fn stop(mut self) -> anyhow::Result<()> {
        self.run("stop")?;
        self.child.wait()?;
        Ok(())
    }
}
