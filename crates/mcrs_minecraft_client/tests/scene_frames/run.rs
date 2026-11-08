use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::client::{McrsClient, workspace_root};
use crate::options;
use crate::scene::{Action, CameraPose, Scene};
use crate::server::VanillaServer;

const JOIN_TIMEOUT: Duration = Duration::from_secs(180);

/// A standing player's eye height.
const EYE_HEIGHT: f64 = 1.62;

const TICK: Duration = Duration::from_millis(50);

/// Where a run reads its inputs and writes its frames.
pub struct Paths {
    pub scenes: PathBuf,
    pub vanilla_frames: PathBuf,
    pub frames: PathBuf,
    pub target: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        let root = workspace_root();
        let target = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .parent()
            .expect("the temporary directory of a test is inside the target directory")
            .to_owned();
        Self {
            scenes: root.join("tools/scenes"),
            vanilla_frames: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/vanilla/frames"),
            frames: target.join("scene-frames"),
            target,
        }
    }

    pub fn report(&self) -> PathBuf {
        self.frames.join("report")
    }
}

/// Where the entities of a frame come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entities {
    /// The server summons them and sends their state: the full mode of CFC-CMP-002.
    Summon,
    /// The client places them from the record of the vanilla client: the render mode of
    /// CFC-CMP-002. The server summons nothing.
    Record,
    /// Leaves out each `summon` command, so the world holds only the player. The background
    /// frames stay the same, since a run moves every entity away before it makes them.
    Skip,
}

/// Runs `scene` on a vanilla server, with the mcrs client as the capturing client, and writes
/// each frame and each background frame to `<frames>/<scene>/<frame>.png`.
pub fn capture(scene: &str, paths: &Paths, entities: Entities) -> Entities {
    let scene = Scene::read(&paths.scenes.join(format!("{scene}.json")));
    // A vanilla server cannot run a pose step, so such a scene is judged in the render mode
    // only (CFC-CMP-002, item 4).
    let has_pose = scene
        .script
        .iter()
        .any(|step| matches!(step, Action::Pose { .. }));
    let entities = if has_pose { Entities::Record } else { entities };
    let record = (entities == Entities::Record).then(|| {
        paths
            .vanilla_frames
            .join(format!("{}.records.json", scene.name))
    });
    let knobs =
        options::knobs(&scene.options).unwrap_or_else(|error| panic!("{}: {error}", scene.name));
    let out = paths.frames.join(&scene.name);
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("the frames of the last run can be removed");
    }
    std::fs::create_dir_all(&out).expect("the frame directory can be created");

    let mut server = VanillaServer::start(&server_jar(paths), &out.join("server"), &scene.world);
    let mut client = McrsClient::start(
        server.port,
        &out,
        scene.resolution,
        &knobs,
        record.as_deref(),
    );
    let deadline = Instant::now() + JOIN_TIMEOUT;
    while !server.wait_for_line("Capture joined the game", Duration::from_secs(1)) {
        client.assert_running("join the server");
        assert!(
            Instant::now() < deadline,
            "the mcrs client did not join in {JOIN_TIMEOUT:?}"
        );
    }
    server.run("gamemode spectator Capture");

    let mut frames = Vec::new();
    for step in &scene.script {
        match step {
            Action::Command { command } => run_command(&mut server, command, entities),
            Action::Commands { commands } => {
                for command in commands {
                    run_command(&mut server, command, entities);
                }
            }
            Action::Wait { wait } => wait_ticks(*wait),
            Action::Step { step } => {
                server.run(&format!("tick step {step}"));
                wait_ticks(step + 10);
            }
            Action::Pose { pose } => eprintln!(
                "{}: a vanilla server cannot give {} the pose {}; the step is skipped",
                scene.name, pose.entity, pose.pose
            ),
            Action::Capture { capture, at } => {
                take(&mut server, &mut client, capture, *at);
                frames.push((capture, *at));
            }
        }
    }

    server.run("tp @e[type=!player] 0 -60 5000");
    wait_ticks(20);
    for (frame, at) in frames {
        take(&mut server, &mut client, &format!("{frame}.background"), at);
    }
    entities
}

fn run_command(server: &mut VanillaServer, command: &str, entities: Entities) {
    if entities != Entities::Summon && command.starts_with("summon ") {
        return;
    }
    server.run(command);
}

fn take(server: &mut VanillaServer, client: &mut McrsClient, frame: &str, at: CameraPose) {
    let [x, y, z] = at.position;
    // A whole number without a decimal point is a block, and `tp` puts the player at its centre.
    server.run(&format!(
        "tp Capture {x:.6} {:.6} {z:.6} {:.6} {:.6}",
        y - EYE_HEIGHT,
        at.yaw,
        at.pitch
    ));
    wait_ticks(10);
    client.capture(frame);
}

fn wait_ticks(ticks: u32) {
    std::thread::sleep(TICK * ticks);
}

/// `MCRS_VANILLA_SERVER_JAR` names the server jar; the default is where
/// `tools/vanilla-server/run.sh fetch` puts the jar of the corpus version. Writing the EULA file
/// of the server needs `MINECRAFT_EULA=true`, as that script does.
fn server_jar(paths: &Paths) -> PathBuf {
    assert!(
        std::env::var("MINECRAFT_EULA").is_ok_and(|accepted| accepted == "true"),
        "the Minecraft EULA is not accepted in this environment; read \
         https://aka.ms/MinecraftEULA, and set MINECRAFT_EULA=true to state the acceptance"
    );
    if let Some(jar) = std::env::var_os("MCRS_VANILLA_SERVER_JAR") {
        return jar.into();
    }
    #[derive(Deserialize)]
    struct Release {
        id: String,
    }
    let release = workspace_root().join("crates/mcrs_minecraft_client_jar/src/release.json");
    let text = std::fs::read_to_string(&release).expect("release.json is readable");
    let release: Release = serde_json::from_str(&text).expect("release.json names its version");
    let jar = paths
        .target
        .join(format!("vanilla-server/{}/server.jar", release.id));
    assert!(
        jar.exists(),
        "no server jar at {}; run tools/vanilla-server/run.sh fetch, or set \
         MCRS_VANILLA_SERVER_JAR",
        jar.display()
    );
    jar
}
