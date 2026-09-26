use std::time::Duration;

use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color_bench::candidates::gpu::Gpu;
use mcrs_minecraft_light_color_bench::fixture::{Fixture, fixtures_dir, oracle};

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

    let gpu = Gpu::new().unwrap_or_else(|e| panic!("{e}"));
    let (levels, elapsed) = gpu.run_one_lane(&scene, centre, torch);

    assert!(
        levels.iter().any(|&level| level > 0),
        "the torch lights nothing"
    );
    assert_eq!(levels, oracle(&scene, centre, torch));
    assert!(elapsed > Duration::ZERO, "the pass took no measurable time");
}
