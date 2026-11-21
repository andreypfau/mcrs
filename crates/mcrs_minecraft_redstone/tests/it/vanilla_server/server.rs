use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

const START_TIMEOUT: Duration = Duration::from_secs(240);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(20);

pub struct VanillaServer {
    process: Child,
    console: ChildStdin,
    lines: Receiver<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    Failed,
}

impl VanillaServer {
    pub fn start(jar: &Path, work: &Path) -> Self {
        if work.exists() {
            std::fs::remove_dir_all(work).expect("the old server directory can be removed");
        }
        std::fs::create_dir_all(work).expect("the server directory can be created");
        let port = free_port();
        std::fs::write(work.join("eula.txt"), "eula=true\n").expect("eula.txt is written");
        std::fs::write(work.join("server.properties"), properties(port))
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
        };
        server.expect_line("Done (", START_TIMEOUT);
        server
    }

    pub fn run(&mut self, command: &str) {
        writeln!(self.console, "{command}").expect("the server console takes a command");
        self.console
            .flush()
            .expect("the server console takes a command");
    }

    pub fn expect_line(&mut self, wanted: &str, timeout: Duration) {
        assert!(
            self.wait_for_line(wanted, timeout),
            "the vanilla server did not print \"{wanted}\" in {timeout:?}"
        );
    }

    pub fn wait_for_line(&mut self, wanted: &str, timeout: Duration) -> bool {
        self.wait_for_any(&[wanted], timeout).is_some()
    }

    fn wait_for_any(&mut self, wanted: &[&str], timeout: Duration) -> Option<usize> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    if let Some(index) = wanted.iter().position(|text| line.contains(text)) {
                        return Some(index);
                    }
                }
                Err(RecvTimeoutError::Timeout) => return None,
                Err(RecvTimeoutError::Disconnected) => panic!("the vanilla server stopped"),
            }
        }
    }

    pub fn test_block(&mut self, pos: (i32, i32, i32), state: &str) -> Verdict {
        let (x, y, z) = pos;
        self.run(&format!("execute if block {x} {y} {z} {state}"));
        match self.wait_for_any(&["Test passed", "Test failed"], ANSWER_TIMEOUT) {
            Some(0) => Verdict::Passed,
            Some(_) => Verdict::Failed,
            None => panic!("the vanilla server did not answer the block test at {x} {y} {z}"),
        }
    }

    pub fn step(&mut self) {
        self.run("tick step 1");
        self.expect_line("Stepping", ANSWER_TIMEOUT);
        self.run("tick query");
        self.expect_line("The game is frozen", ANSWER_TIMEOUT);
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

fn properties(port: u16) -> String {
    format!(
        "online-mode=false\n\
         server-ip=127.0.0.1\n\
         server-port={port}\n\
         level-type=minecraft\\:flat\n\
         level-seed=4\n\
         generate-structures=false\n\
         gamemode=creative\n\
         force-gamemode=true\n\
         view-distance=6\n\
         simulation-distance=6\n\
         spawn-protection=0\n\
         network-compression-threshold=-1\n\
         enforce-secure-profile=false\n\
         enable-rcon=false\n\
         enable-query=false\n\
         white-list=false\n\
         enforce-whitelist=false\n\
         max-tick-time=-1\n"
    )
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a loopback port is free")
        .port()
}

fn forward_lines(output: impl std::io::Read + Send + 'static, copy: PathBuf) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut file = File::create(copy).ok();
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { return };
            if let Some(file) = file.as_mut() {
                let _ = writeln!(file, "{line}");
            }
            let _ = sender.send(line);
        }
    });
    receiver
}

#[expect(
    clippy::disallowed_methods,
    reason = "the harness reads the acceptance of the EULA and the jar path from the environment"
)]
pub fn server_jar() -> Option<PathBuf> {
    if !std::env::var("MINECRAFT_EULA").is_ok_and(|accepted| accepted == "true") {
        eprintln!(
            "MINECRAFT_EULA=true is not set; read https://aka.ms/MinecraftEULA and set it to \
             state the acceptance. The vanilla differential test is skipped."
        );
        return None;
    }
    if let Some(jar) = std::env::var_os("MCRS_VANILLA_SERVER_JAR") {
        return Some(jar.into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let release = root.join("crates/mcrs_minecraft_client_jar/src/release.json");
    let text = std::fs::read_to_string(&release).expect("release.json is readable");
    let id = text
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let value = line.strip_prefix("\"id\": \"")?;
            value.split('"').next()
        })
        .expect("release.json names its version");
    let jar = root.join(format!("target/vanilla-server/{id}/server.jar"));
    if jar.exists() {
        Some(jar)
    } else {
        eprintln!(
            "no server jar at {}; run tools/vanilla-server/run.sh fetch, or set \
             MCRS_VANILLA_SERVER_JAR. The vanilla differential test is skipped.",
            jar.display()
        );
        None
    }
}
