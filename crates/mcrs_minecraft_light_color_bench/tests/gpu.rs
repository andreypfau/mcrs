use std::time::Duration;

use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::region::{Palette, Region, section_output};
use mcrs_minecraft_light_color_bench::candidates::gpu::run;
use mcrs_minecraft_light_color_bench::candidates::{Stages, mismatch};
use mcrs_minecraft_light_color_bench::fixture::{Fixture, fixtures_dir, oracle, scene};

#[test]
fn one_gpu_lane_matches_the_server_rule() {
    let fixture = Fixture::read(&fixtures_dir().join("overlap.json.gz"));
    let torch = fixture
        .palette
        .iter()
        .find(|state| state.state.to_string() == "minecraft:torch")
        .map(|state| LightType(state.light_type))
        .expect("the overlap scene holds a torch");
    let scene = fixture.scene("overlap");
    let centre = SectionPos::new(scene.origin.x + 2, scene.origin.y + 2, scene.origin.z + 2);

    let mut stages = Stages::default();
    let outcome = run(&scene, centre, &mut stages).expect("light reaches the centre section");
    let (_, levels) = outcome
        .lanes
        .expect("the GPU keeps its lanes")
        .into_iter()
        .find(|(t, _)| *t == torch)
        .expect("the torch has a lane");

    assert!(
        levels.iter().any(|&level| level > 0),
        "the torch lights nothing"
    );
    assert_eq!(levels, oracle(&scene, centre, torch));
    assert!(
        stages.propagation > Duration::ZERO,
        "the pass took no measurable time"
    );
}

#[test]
fn every_gpu_lane_matches_the_server_rule_on_every_scene() {
    for name in ["nether_lava", "caves", "overlap"] {
        let scene = scene(name);
        let mut lit = 0;
        for section in scene.inner() {
            let (min, size) = section_output(section);
            let region = Region::new(min, size, scene.bounds, &scene.registry, scene.cells());
            let types = Palette::of(&region, &scene.registry, &scene.colours).types;
            let mut stages = Stages::default();
            let Some(outcome) = run(&scene, section, &mut stages) else {
                assert!(
                    types.is_empty(),
                    "{name}: {section:?} has light but no outcome"
                );
                continue;
            };
            lit += 1;
            let lanes = outcome.lanes.expect("the GPU keeps its lanes");
            let lane_types: Vec<LightType> = lanes.iter().map(|(t, _)| *t).collect();
            assert_eq!(lane_types, types, "{name}: {section:?} has another palette");
            if let Some(first) = mismatch(section, &lanes, |t| oracle(&scene, section, t)) {
                panic!("{name}: section, cell, type, lane, relax: {first}");
            }
            assert!(
                stages.propagation > Duration::ZERO,
                "{name}: an untimed pass"
            );
        }
        assert!(lit > 0, "{name} has no section the GPU lights");
    }
}
