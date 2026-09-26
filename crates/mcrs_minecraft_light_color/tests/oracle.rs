mod common;

use common::*;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::propagate::{Lanes, colour_section};
use mcrs_minecraft_light_color::region::{Palette, Region, lane_bytes};
use mcrs_minecraft_light_color::resolve::{light_weight, resolve};
use proptest::prelude::*;

const CENTRE: SectionPos = SectionPos(bevy_math::IVec3::new(0, 4, 0));

#[test]
fn a_torch_lane_matches_the_server_rule() {
    let world = torch_on_a_floor(CENTRE);
    let computed = compute(&world);
    assert_eq!(computed.palette.types, vec![TORCH_TYPE]);
    assert_eq!(lane_mismatch(&world, &computed, TORCH_TYPE), None);
}

#[test]
fn a_lone_torch_resolves_to_its_colour_and_dark_cells_to_the_default_weight() {
    let world = torch_on_a_floor(CENTRE);
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

#[test]
fn lane_bytes_follow_the_palette_size() {
    let widths = [
        (0, 0),
        (1, 1),
        (2, 2),
        (3, 4),
        (5, 8),
        (8, 8),
        (9, 16),
        (16, 16),
        (17, 24),
        (40, 40),
    ];
    for (types, bytes) in widths {
        assert_eq!(lane_bytes(types), bytes, "{types} types");
    }
}

#[test]
fn sixteen_seventeen_and_forty_types_each_match_the_server_rule() {
    for (count, bytes) in [(16, 16), (17, 24), (40, 40)] {
        let world = many_types(CENTRE, count);
        let computed = assert_lanes_match(&world);
        assert_eq!(computed.palette.types.len(), count as usize);
        assert_eq!(computed.lanes.bytes, bytes, "{count} types");
    }
}

#[test]
fn an_empty_region_has_no_colour() {
    let world = Neighbourhood::air(CENTRE);
    let colour = colour_section(CENTRE, world.bounds, &registry(), &colours(), world.cells());
    assert!(colour.is_none());
    assert_eq!(compute(&world).lanes.bytes, 0);
}

#[test]
fn opaque_emitters_keep_their_own_light() {
    let (world, glowstone) = glowstone_among_torches(CENTRE);
    let computed = assert_lanes_match(&world);
    assert_eq!(computed.palette.types, vec![LightType::DEFAULT, TORCH_TYPE]);
    assert_eq!(computed.lanes.bytes, 2);
    let region = &computed.region;
    let cell = region.index(
        glowstone.x - region.min.x,
        glowstone.y - region.min.y,
        glowstone.z - region.min.z,
    );
    let default = computed.palette.lane(LightType::DEFAULT).unwrap();
    let torch = computed.palette.lane(TORCH_TYPE).unwrap();
    assert_eq!(computed.lanes.level(cell, default), 15);
    assert_eq!(computed.lanes.level(cell, torch), 0);
}

fn resolve_one(levels: &[(LightType, u8)]) -> [u8; 4] {
    let region = Region {
        min: BlockPos::new(0, 0, 0),
        size: 1,
        blocks: Box::new([common::AIR]),
    };
    let palette = Palette {
        types: levels.iter().map(|&(t, _)| t).collect(),
    };
    let lanes = Lanes {
        bytes: levels.len(),
        levels: levels.iter().map(|&(_, level)| level).collect(),
    };
    resolve(&region, &lanes, &palette, &colours(), region.min, 1)[0]
}

fn assert_near(got: [u8; 3], want: [f32; 3]) {
    for (g, w) in got.iter().zip(want) {
        assert!((*g as f32 - w).abs() <= 1.0, "{got:?} is not {want:?}");
    }
}

#[test]
fn resolve_mixes_known_colours_by_the_light_weight() {
    let (torch, soul) = (colour(TORCH_TYPE), colour(SOUL_TYPE));
    let mixed = resolve_one(&[(TORCH_TYPE, 10), (SOUL_TYPE, 10)]);
    let mean = [0, 1, 2].map(|i| (torch[i] as f32 + soul[i] as f32) / 2.0);
    assert_near([mixed[0], mixed[1], mixed[2]], mean);
    assert_eq!(mixed[3], 0);

    assert_eq!(resolve_one(&[(LightType::DEFAULT, 12)]), [0, 0, 0, 255]);

    let half = resolve_one(&[(LightType::DEFAULT, 9), (TORCH_TYPE, 9)]);
    assert!(half[3] == 127 || half[3] == 128, "{half:?}");
    assert_near([half[0], half[1], half[2]], torch.map(|c| c as f32 / 2.0));

    let brighter = resolve_one(&[(TORCH_TYPE, 15), (SOUL_TYPE, 5)]);
    let (w15, w5) = (light_weight(15), light_weight(5));
    let weighted = [0, 1, 2].map(|i| (torch[i] as f32 * w15 + soul[i] as f32 * w5) / (w15 + w5));
    assert_near([brighter[0], brighter[1], brighter[2]], weighted);

    assert_eq!(
        resolve_one(&[(TORCH_TYPE, 0), (LightType::DEFAULT, 0)]),
        [0, 0, 0, 255]
    );
    assert_eq!(resolve_one(&[]), [0, 0, 0, 255]);
}

#[test]
fn nothing_is_retained_between_recomputes() {
    let (a, b) = (torch_on_a_floor(CENTRE), soul_and_lava_by_a_wall(CENTRE));
    let (registry, colours) = (registry(), colours());
    let run = |world: &Neighbourhood| {
        colour_section(CENTRE, world.bounds, &registry, &colours, world.cells()).unwrap()
    };
    let (a_first, b_second) = (run(&a), run(&b));
    let (b_first, a_second) = (run(&b), run(&a));
    assert!(a_first == a_second);
    assert!(b_first == b_second);
    assert!(a_first != b_first);
}

#[test]
fn a_block_id_past_the_registry_is_opaque_and_uncoloured() {
    let (world, beside) = unknown_blocks_by_a_torch(CENTRE);
    let computed = assert_lanes_match(&world);
    assert_eq!(computed.palette.types, vec![TORCH_TYPE]);
    let region = &computed.region;
    let cell = region.index(
        beside.x - region.min.x,
        beside.y - region.min.y,
        beside.z - region.min.z,
    );
    assert_eq!(computed.lanes.level(cell, 0), 0);
    assert!(colour_section(CENTRE, world.bounds, &registry(), &colours(), world.cells()).is_some());
}

#[test]
fn every_brick_matches_the_region_edge_costs() {
    let worlds = [
        torch_on_a_floor(CENTRE),
        soul_and_lava_by_a_wall(CENTRE),
        many_types(CENTRE, 40),
        glowstone_among_torches(CENTRE).0,
        unknown_blocks_by_a_torch(CENTRE).0,
        Neighbourhood::air(CENTRE),
    ];
    let (registry, colours) = (registry(), colours());
    for world in &worlds {
        let found = brick_mismatch(CENTRE, world.bounds, &registry, &colours, world.cells());
        assert_eq!(found, None);
    }
}

proptest! {
    #[test]
    fn every_lane_matches_the_server_rule_on_random_regions(parts in world_parts()) {
        let world = random_world(CENTRE, parts);
        let computed = compute(&world);
        for &t in &computed.palette.types {
            prop_assert_eq!(lane_mismatch(&world, &computed, t), None);
        }
        let found = brick_mismatch(CENTRE, world.bounds, &registry(), &colours(), world.cells());
        prop_assert_eq!(found, None);
    }
}
