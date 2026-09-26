mod common;

use common::*;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::region::{Palette, Region};
use mcrs_minecraft_light_color::resolve::{Lanes, light_weight, resolve};
use proptest::prelude::*;

const CENTRE: SectionPos = SectionPos(bevy_math::IVec3::new(0, 4, 0));

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
    fn every_brick_matches_the_region_edge_costs_on_random_regions(parts in world_parts()) {
        let world = random_world(CENTRE, parts);
        let found = brick_mismatch(CENTRE, world.bounds, &registry(), &colours(), world.cells());
        prop_assert_eq!(found, None);
    }
}
