//! Runs the scenes of `tools/scenes` with the mcrs client and judges its frames against the
//! frames of the vanilla client, with the FLIP error.
//!
//! The scene tests need java, the vanilla server jar (`tools/vanilla-server/run.sh fetch`),
//! `MINECRAFT_EULA=true` and a GPU:
//!
//! ```sh
//! tools/vanilla-server/run.sh fetch
//! MINECRAFT_EULA=true cargo test -p mcrs_minecraft_client --test scene_frames -- \
//!     --ignored --test-threads=1
//! ```
//!
//! The frames go to `target/scene-frames/<scene>/<frame>.png`, and the report of each scene and
//! the FLIP error images to `target/scene-frames/report/`. `judge_captured_frames` then judges
//! the frames of all scenes again, with no server, into `report/all.txt`.
#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "the test finds the server jar and the EULA acceptance through the environment"
)]

mod client;
mod judge;
mod options;
mod run;
mod scene;
mod server;

use run::{Entities, Paths};

const SCENES: [&str; 6] = [
    "pig_idle",
    "pig_walk",
    "pig_lineup",
    "pig_hurt_death",
    "frog_states",
    "pig_grid",
];

/// The scenes run without their entities unless `MCRS_SCENE_ENTITIES=summon`: the client cannot
/// decode every entity move of a vanilla server yet, and mcrs draws no entity, so only the
/// background frames are judged.
fn capture_and_judge(scene: &str) {
    let paths = Paths::new();
    let entities = if std::env::var("MCRS_SCENE_ENTITIES").is_ok_and(|value| value == "summon") {
        Entities::Summon
    } else {
        Entities::Skip
    };
    run::capture(scene, &paths, entities);
    let scores = judge::judge(scene, &paths);
    judge::conclude(scene, &scores, &paths);
}

#[test]
#[ignore = "needs java, the vanilla server jar, the EULA acceptance and a GPU"]
fn pig_idle() {
    capture_and_judge("pig_idle");
}

#[test]
#[ignore = "needs java, the vanilla server jar, the EULA acceptance and a GPU"]
fn pig_walk() {
    capture_and_judge("pig_walk");
}

#[test]
#[ignore = "needs java, the vanilla server jar, the EULA acceptance and a GPU"]
fn pig_lineup() {
    capture_and_judge("pig_lineup");
}

#[test]
#[ignore = "needs java, the vanilla server jar, the EULA acceptance and a GPU"]
fn pig_hurt_death() {
    capture_and_judge("pig_hurt_death");
}

#[test]
#[ignore = "needs java, the vanilla server jar, the EULA acceptance and a GPU"]
fn frog_states() {
    capture_and_judge("frog_states");
}

#[test]
#[ignore = "needs java, the vanilla server jar, the EULA acceptance and a GPU"]
fn pig_grid() {
    capture_and_judge("pig_grid");
}

/// Judges the frames that the scene tests left, without a server.
#[test]
#[ignore = "needs the frames of a run of the scene tests"]
fn judge_captured_frames() {
    let paths = Paths::new();
    let scores: Vec<judge::Score> = SCENES
        .iter()
        .flat_map(|scene| judge::judge(scene, &paths))
        .collect();
    judge::conclude("all", &scores, &paths);
}
