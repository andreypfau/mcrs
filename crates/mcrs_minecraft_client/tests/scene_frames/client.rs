use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::options::Knob;

const FRAME_TIMEOUT: Duration = Duration::from_secs(180);

/// The mcrs client, drawing off screen into frames that the harness requests by name.
pub struct McrsClient {
    process: Child,
    dir: PathBuf,
}

impl McrsClient {
    /// Joins the server on `port` as the player `Capture`, and writes frames of `size` pixels
    /// into `dir`.
    /// With `record`, the client places the entities of each frame from that record of the
    /// vanilla client.
    pub fn start(
        port: u16,
        dir: &Path,
        size: [u32; 2],
        knobs: &[Knob],
        record: Option<&Path>,
    ) -> Self {
        let log = File::create(dir.join("client.log")).expect("the client log can be created");
        let mut command = Command::new(client_binary());
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("MCRS_") {
                command.env_remove(name);
            }
        }
        command
            .current_dir(workspace_root())
            .env("MCRS_SERVER", format!("127.0.0.1:{port}"))
            .env("MCRS_USERNAME", "Capture")
            .env("MCRS_SCENE_DIR", dir)
            .env("MCRS_RESOLUTION", format!("{}x{}", size[0], size[1]))
            .stdout(log.try_clone().expect("the client log can be shared"))
            .stderr(log);
        if let Some(record) = record {
            command.env("MCRS_SCENE_RECORD", record);
        }
        for knob in knobs {
            command.env(format!("MCRS_{}", knob.name), &knob.value);
        }
        let process = command.spawn().expect("the mcrs client starts");
        Self {
            process,
            dir: dir.to_owned(),
        }
    }

    /// Asks for the frame `name` and waits until the client has written it.
    pub fn capture(&mut self, name: &str) -> PathBuf {
        let frame = self.dir.join(format!("{name}.png"));
        let request = self.dir.join("request");
        std::fs::write(&request, name).expect("the frame request is written");
        let deadline = Instant::now() + FRAME_TIMEOUT;
        let unsettled = self.dir.join(format!("{name}.unsettled"));
        while !frame.exists() {
            assert!(
                !unsettled.exists(),
                "the frame {name} did not settle in 60 s (CFC-RUN-004)"
            );
            self.assert_running(&format!("write {name}"));
            assert!(
                Instant::now() < deadline,
                "the mcrs client did not write {name} in {FRAME_TIMEOUT:?}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        frame
    }

    pub fn assert_running(&mut self, task: &str) {
        if let Ok(Some(status)) = self.process.try_wait() {
            panic!("the mcrs client stopped with {status} before it could {task}; see client.log");
        }
    }
}

impl Drop for McrsClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// `MCRS_SCENE_CLIENT` names another client binary, for example a release build; the default is
/// the binary that cargo built with this test.
fn client_binary() -> OsString {
    std::env::var_os("MCRS_SCENE_CLIENT")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_mcrs_minecraft_client").into())
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
