#[path = "../../mcrs_minecraft_light_color/tests/common/mod.rs"]
mod common;

use mcrs_minecraft_light_color_bench::candidates::{CANDIDATES, Stages, mismatch};
use mcrs_minecraft_light_color_bench::fixture::{oracle, scene};

fn assert_candidates_match_relax(name: &str) {
    let scene = scene(name);
    for section in scene.inner() {
        let (registry, colours) = (&scene.registry, &scene.colours);
        let found = common::brick_mismatch(section, scene.bounds, registry, colours, scene.cells());
        assert_eq!(found, None, "{name}");
    }
    for candidate in CANDIDATES.iter().filter(|c| c.exact) {
        let mut lit = 0;
        for section in scene.inner() {
            let Some(outcome) = (candidate.run)(&scene, section, &mut Stages::default()) else {
                continue;
            };
            lit += 1;
            let lanes = outcome.lanes.expect("an exact candidate keeps its lanes");
            if let Some(first) = mismatch(section, &lanes, |t| oracle(&scene, section, t)) {
                panic!(
                    "{name}, {}: section, cell, type, lane, relax: {first}",
                    candidate.name
                );
            }
        }
        assert!(lit > 0, "{name} has no section {} lights", candidate.name);
    }
}

#[test]
fn the_nether_fixture_matches_the_server_rule() {
    assert_candidates_match_relax("nether_lava");
}

#[test]
fn the_caves_fixture_matches_the_server_rule() {
    assert_candidates_match_relax("caves");
}

#[test]
fn the_overlap_scene_matches_the_server_rule() {
    assert_candidates_match_relax("overlap");
}
