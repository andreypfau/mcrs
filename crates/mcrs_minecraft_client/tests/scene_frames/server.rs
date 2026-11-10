use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::scene::World;

const START_TIMEOUT: Duration = Duration::from_secs(300);

/// A vanilla dedicated server that runs the world of one scene. The harness types commands at
/// its console, so each command has the permissions of the console.
pub struct VanillaServer {
    process: Child,
    console: ChildStdin,
    lines: Receiver<String>,
    pub port: u16,
}

impl VanillaServer {
    /// Starts `jar` in `work`, which holds nothing else, and waits until the world is ready.
    pub fn start(jar: &Path, work: &Path, world: &World) -> Self {
        assert_eq!(
            world.kind, "flat_default",
            "the harness only knows the default flat world"
        );
        if work.exists() {
            std::fs::remove_dir_all(work).expect("the old server directory can be removed");
        }
        std::fs::create_dir_all(work).expect("the server directory can be created");
        let port = free_port();
        std::fs::write(work.join("eula.txt"), "eula=true\n").expect("eula.txt is written");
        std::fs::write(
            work.join("server.properties"),
            properties(port, &world.seed, world.generate_structures),
        )
        .expect("server.properties is written");

        let log = File::create(work.join("server.log")).expect("the server log can be created");
        let mut process = Command::new("java")
            .args(["-Xmx2G", "-jar"])
            .arg(jar)
            .arg("nogui")
            .current_dir(work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()
            .expect("java starts; the harness needs java on the PATH");
        let console = process.stdin.take().expect("the console is piped");
        let stdout = process.stdout.take().expect("the output is piped");
        let lines = forward_lines(stdout, work.join("server.out"));
        let mut server = Self {
            process,
            console,
            lines,
            port,
        };
        server.expect_line("Done (", START_TIMEOUT);
        for (rule, value) in &world.game_rules {
            server.run(&format!("gamerule minecraft:{rule} {value}"));
        }
        server
    }

    pub fn run(&mut self, command: &str) {
        writeln!(self.console, "{command}").expect("the server console takes a command");
        self.console
            .flush()
            .expect("the server console takes a command");
    }

    /// Waits for a line of the console output that contains `wanted`.
    pub fn expect_line(&mut self, wanted: &str, timeout: Duration) {
        assert!(
            self.wait_for_line(wanted, timeout),
            "the vanilla server did not print \"{wanted}\" in {timeout:?}"
        );
    }

    /// Answers whether a line of the console output that contains `wanted` came within
    /// `timeout`.
    pub fn wait_for_line(&mut self, wanted: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) if line.contains(wanted) => return true,
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => return false,
                Err(RecvTimeoutError::Disconnected) => panic!("the vanilla server stopped"),
            }
        }
    }
}

impl Drop for VanillaServer {
    fn drop(&mut self) {
        let _ = writeln!(self.console, "stop");
        let _ = self.console.flush();
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.process.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn properties(port: u16, seed: &str, generate_structures: bool) -> String {
    format!(
        "online-mode=false\n\
         server-ip=127.0.0.1\n\
         server-port={port}\n\
         level-type=minecraft\\:flat\n\
         level-seed={seed}\n\
         generate-structures={generate_structures}\n\
         gamemode=spectator\n\
         force-gamemode=true\n\
         view-distance=10\n\
         simulation-distance=10\n\
         spawn-protection=0\n\
         network-compression-threshold=-1\n\
         enforce-secure-profile=false\n\
         enable-rcon=false\n\
         enable-query=false\n\
         white-list=false\n\
         enforce-whitelist=false\n"
    )
}

/// The port is free when this returns; nothing else on the machine is expected to take it before
/// the server binds it.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a loopback port is free")
        .port()
}

/// Copies each line of `output` to the file `copy` and to the returned channel.
fn forward_lines(output: impl std::io::Read + Send + 'static, copy: PathBuf) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut file = File::create(copy).ok();
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { return };
            if let Some(file) = file.as_mut() {
                let _ = writeln!(file, "{line}");
            }
            // The pipe is drained to the end even with no reader, or the server blocks on it.
            let _ = sender.send(line);
        }
    });
    receiver
}
