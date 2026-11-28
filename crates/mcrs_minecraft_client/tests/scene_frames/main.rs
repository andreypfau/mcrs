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

fn capture_and_judge(scene: &str) {
    let paths = Paths::new();
    let entities = match std::env::var("MCRS_SCENE_ENTITIES").as_deref() {
        Ok("summon") => Entities::Summon,
        Ok("record") => Entities::Record,
        _ => Entities::Skip,
    };
    let entities = run::capture(scene, &paths, entities);
    println!("{scene}: entities {entities:?}");
    let scores = judge::judge(scene, &paths);
    judge::conclude(scene, &scores, &paths);
}

#[test]
#[ignore = "needs java, the vanilla server jar, the EULA acceptance and a GPU"]
fn scenes() {
    for scene in SCENES {
        capture_and_judge(scene);
    }
}

#[test]
fn every_scene_file_is_a_scene_the_harness_runs() {
    let paths = Paths::new();
    let files = std::fs::read_dir(&paths.scenes)
        .expect("the scene directory can be read")
        .filter(|entry| {
            entry
                .as_ref()
                .is_ok_and(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        })
        .count();
    assert_eq!(files, SCENES.len(), "every scene file is listed in SCENES");
    for name in SCENES {
        let scene = scene::Scene::read(&paths.scenes.join(format!("{name}.json")));
        assert_eq!(scene.name, name);
        options::client_options(&scene.options).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

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
