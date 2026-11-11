use crate::client_join::StopOnDrop;
use crate::configuration_registries::ASSETS;
use bevy_app::{App, AppExit, PluginGroup, Update};
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_engine::banlist::BanList;
use mcrs_minecraft_engine::coordinator::ops::{OpList, OpListEntry};
use mcrs_minecraft_engine_server::{BoundAddress, run_server_loop};
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_probe::{KnownDifference, Scenario, Trace};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_vanilla_light::LightingPlugin;
use mcrs_minecraft_vanilla_worldgen::WorldGenPlugin;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const PROBE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/mcrs_minecraft_probe"
);

// The embedded server answers on loopback in a few ticks; the recorded trace keeps 1 s.
const QUIET: Duration = Duration::from_millis(250);

fn differences(
    scenario: &Scenario,
    vanilla: &Trace,
    known: &[KnownDifference],
    mcrs: &Trace,
) -> String {
    let mut text = String::new();
    for (index, difference) in known.iter().enumerate() {
        if known[..index]
            .iter()
            .any(|earlier| earlier.step.id == difference.step.id)
        {
            writeln!(
                text,
                "the step {} has two known differences",
                difference.step.id
            )
            .unwrap();
        }
        if !scenario
            .steps
            .iter()
            .any(|step| step.id == difference.step.id)
        {
            writeln!(
                text,
                "the known difference {} names no step of the scenario",
                difference.step.id
            )
            .unwrap();
        }
    }
    if vanilla.version != mcrs.version {
        writeln!(
            text,
            "version\n  vanilla: {:?}\n  mcrs: {:?}",
            vanilla.version, mcrs.version
        )
        .unwrap();
    }
    if vanilla.steps.len() != mcrs.steps.len() {
        writeln!(
            text,
            "step count\n  vanilla: {}\n  mcrs: {}",
            vanilla.steps.len(),
            mcrs.steps.len()
        )
        .unwrap();
    }
    for (reference, found) in vanilla.steps.iter().zip(&mcrs.steps) {
        let listed = known
            .iter()
            .find(|difference| difference.step.id == reference.id);
        let (origin, expected) = match listed {
            Some(difference) => ("known difference", &difference.step),
            None => ("vanilla", reference),
        };
        let expected_json = serde_json::to_value(expected).unwrap();
        let found_json = serde_json::to_value(found).unwrap();
        if expected_json != found_json {
            writeln!(
                text,
                "step {}\n  {origin}: {expected_json}\n  mcrs: {found_json}",
                reference.id
            )
            .unwrap();
        }
    }
    text
}

#[test]
fn the_ban_scenario_on_the_embedded_server_matches_the_vanilla_trace() {
    let probe = Path::new(PROBE);
    let scenario: Scenario =
        serde_json::from_slice(&fs::read(probe.join("scenarios/bans.json")).unwrap()).unwrap();
    let vanilla: Trace =
        serde_json::from_slice(&fs::read(probe.join("traces/bans.json")).unwrap()).unwrap();
    let known: Vec<KnownDifference> =
        serde_json::from_slice(&fs::read(probe.join("traces/bans.mcrs.json")).unwrap()).unwrap();

    let directory = std::env::temp_dir().join(format!("mcrs-probe-bans-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let ban_file = directory.join("banned-players.json");
    fs::write(
        &ban_file,
        serde_json::to_vec(&scenario.target.banned_players).unwrap(),
    )
    .unwrap();

    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.network.announce_on_lan = false;
    plugins.network.banlist = BanList::read(&ban_file);
    plugins.worlds.ops = OpList::new(
        scenario
            .target
            .operators
            .iter()
            .map(|name| OpListEntry::owner(offline_player_uuid(name), name.clone())),
    );
    let mut app = App::new();
    app.add_plugins(
        plugins
            .build()
            .disable::<WorldGenPlugin>()
            .disable::<LightingPlugin>(),
    );
    let stop = StopOnDrop(Arc::new(AtomicBool::new(false)));
    let stopped = stop.0.clone();
    app.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stopped.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });
    let address = app.world().resource::<BoundAddress>().0;
    let (finished_send, finished_recv) = std::sync::mpsc::channel();
    let server = mcrs_minecraft_engine_server::spawn_server_thread(app, move |app| {
        run_server_loop(app);
        finished_send.send(()).ok();
    })
    .expect("the server thread starts");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mcrs = runtime.block_on(mcrs_minecraft_probe::run(&scenario, address, QUIET, || {
        Ok(serde_json::from_slice(&fs::read(&ban_file)?)?)
    }));
    drop(stop);
    finished_recv
        .recv_timeout(Duration::from_secs(30))
        .expect("the embedded server did not stop after AppExit");
    server.join().unwrap();
    fs::remove_dir_all(&directory).ok();

    let differences = differences(&scenario, &vanilla, &known, &mcrs.unwrap());
    assert!(
        differences.is_empty(),
        "the embedded server differs from the vanilla trace and the known differences:\n{differences}"
    );
}
