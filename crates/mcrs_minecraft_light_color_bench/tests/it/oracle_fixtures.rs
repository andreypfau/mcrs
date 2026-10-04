use crate::common;
use mcrs_minecraft_light_color_bench::fixture::scene;

#[test]
fn every_fixture_brick_matches_the_region_edge_costs() {
    for name in ["nether_lava", "caves", "overlap"] {
        let scene = scene(name);
        for section in scene.inner() {
            let (registry, colours) = (&scene.registry, &scene.colours);
            let found =
                common::brick_mismatch(section, scene.bounds, registry, colours, scene.cells());
            assert_eq!(found, None, "{name}");
        }
    }
}
