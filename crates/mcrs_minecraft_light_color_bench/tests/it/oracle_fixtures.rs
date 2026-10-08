use crate::common;
use mcrs_minecraft_light_color_bench::fixture::scene;

#[test]
fn one_overlap_brick_matches_the_region_edge_costs() {
    let scene = scene("overlap");
    let section = scene
        .inner()
        .next()
        .expect("the overlap scene has an inner section");
    let (registry, colours) = (&scene.registry, &scene.colours);
    let found = common::brick_mismatch(section, scene.bounds, registry, colours, scene.cells());
    assert_eq!(found, None);
}

mod exhaustive {
    use super::*;

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
}
