use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::time::Duration;
use std::time::Instant;

pub struct Sandbox {
    pub state: PathBuf,
}

impl Sandbox {
    pub fn new(name: &str) -> Self {
        let state = std::env::temp_dir().join(format!("mcrs_mcp_it_{}_{name}", std::process::id()));
        std::fs::create_dir_all(&state).unwrap();
        Self { state }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_mcp"));
        command.env("TMPDIR", &self.state);
        command
    }

    pub fn run(&self, tool: &str, arguments: &serde_json::Value) -> Output {
        self.command()
            .args([tool, &arguments.to_string()])
            .output()
            .unwrap()
    }

    pub fn ok(&self, tool: &str, arguments: &serde_json::Value) -> String {
        let output = self.run(tool, arguments);
        assert!(
            output.status.success(),
            "{tool} {arguments}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.state);
    }
}

pub fn fake_workdir() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fake_workdir").to_owned()
}

pub fn alive(pid: u64) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .unwrap()
        .status
        .success()
}

pub fn gone_within(pid: u64, timeout: Duration) -> bool {
    let start = Instant::now();
    while alive(pid) {
        if start.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}
