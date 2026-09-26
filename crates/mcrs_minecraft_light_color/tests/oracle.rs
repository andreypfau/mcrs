mod common;

use common::*;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::propagate::colour_section;

const CENTRE: SectionPos = SectionPos(bevy_math::IVec3::new(0, 4, 0));

fn torch_on_a_floor() -> Neighbourhood {
    let mut world = Neighbourhood::air(CENTRE);
    let floor = CENTRE.y * 16 + 2;
    for z in -16..32 {
        for x in -16..32 {
            world.set(BlockPos::new(x, floor, z), STONE);
        }
    }
    world.set(BlockPos::new(5, floor + 1, 5), TORCH);
    world.set(BlockPos::new(6, floor + 1, 5), GLASS);
    world
}

#[test]
fn a_torch_lane_matches_the_server_rule() {
    let world = torch_on_a_floor();
    let computed = compute(&world);
    assert_eq!(computed.palette.types, vec![TORCH_TYPE]);
    assert_eq!(lane_mismatch(&world, &computed, TORCH_TYPE), None);
}

#[test]
fn a_lone_torch_resolves_to_its_colour_and_dark_cells_to_the_default_weight() {
    let world = torch_on_a_floor();
    let registry = registry();
    let texels = colour_section(CENTRE, world.bounds, &registry, &colours(), world.cells())
        .expect("a torch gives the section colour");
    let levels = oracle(&world, TORCH_TYPE);
    let [r, g, b] = colour(TORCH_TYPE);
    assert!(levels.contains(&14) && levels.contains(&0));
    for ((pos, texel), level) in output_positions(CENTRE).zip(texels.iter()).zip(levels) {
        let expected = if level > 0 {
            [r, g, b, 0]
        } else {
            [0, 0, 0, 255]
        };
        assert_eq!(*texel, expected, "at {pos} with level {level}");
    }
}
