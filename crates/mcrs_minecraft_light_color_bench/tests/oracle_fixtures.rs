use mcrs_minecraft_light_color_bench::candidates::{Stages, mismatch, reference};
use mcrs_minecraft_light_color_bench::fixture::{oracle, scene};

fn assert_reference_matches_relax(name: &str) {
    let scene = scene(name);
    let mut lit = 0;
    for section in scene.inner() {
        let Some(outcome) = reference::run(&scene, section, &mut Stages::default()) else {
            continue;
        };
        lit += 1;
        let lanes = outcome.lanes.expect("the reference keeps its lanes");
        if let Some(first) = mismatch(section, &lanes, |t| oracle(&scene, section, t)) {
            panic!("{name}: section, cell, type, lane, relax: {first}");
        }
    }
    assert!(lit > 0, "{name} has no lit inner section");
}

#[test]
fn the_nether_fixture_matches_the_server_rule() {
    assert_reference_matches_relax("nether_lava");
}

#[test]
fn the_caves_fixture_matches_the_server_rule() {
    assert_reference_matches_relax("caves");
}

#[test]
fn the_overlap_scene_matches_the_server_rule() {
    assert_reference_matches_relax("overlap");
}
