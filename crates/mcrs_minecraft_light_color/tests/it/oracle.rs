use std::collections::{BTreeMap, HashMap};

use crate::common::*;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::layout::{Emitter, PackedBrick, neighbours, pack, reaching_types};
use mcrs_minecraft_light_color::region::{Palette, Region, section_bricks, section_output};
use mcrs_minecraft_light_color::resolve::{Lanes, resolve};
use proptest::prelude::*;

const CENTRE: SectionPos = SectionPos(bevy_math::IVec3::new(0, 4, 0));

fn resolve_one(levels: &[(LightType, u8)]) -> [u8; 4] {
    let region = Region {
        min: BlockPos::new(0, 0, 0),
        size: 1,
        blocks: Box::new([crate::common::AIR]),
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
    let distance = |to: [u8; 3]| {
        (0..3)
            .map(|i| brighter[i].abs_diff(to[i]) as u32)
            .sum::<u32>()
    };
    assert!(
        distance(torch) < distance(soul),
        "{brighter:?} leans to the dimmer light"
    );

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

#[test]
fn a_packed_brick_holds_the_section_bricks() {
    let worlds = [
        torch_on_a_floor(CENTRE),
        soul_and_lava_by_a_wall(CENTRE),
        many_types(CENTRE, 40),
        glowstone_among_torches(CENTRE).0,
        unknown_blocks_by_a_torch(CENTRE).0,
        Neighbourhood::air(CENTRE),
    ];
    let (registry, colours) = (registry(), colours());
    let mut emitting = 0;
    for world in &worlds {
        for section in neighbours(CENTRE) {
            let bricks = section_bricks(section, world.bounds, &registry, &colours, world.cells());
            let packed = pack(&bricks);
            let mut boxes: BTreeMap<(LightType, u8), ([u8; 3], [u8; 3])> = BTreeMap::new();
            for (local, &word) in packed.words.iter().enumerate() {
                let seed = bricks.seeds[local];
                assert_eq!(
                    word & 15,
                    bricks.entry[local] as u32,
                    "{section:?} cell {local}"
                );
                assert_eq!(
                    word >> 4 & 7,
                    bricks.veto[local] as u32,
                    "{section:?} cell {local}"
                );
                assert_eq!(word >> 7 & 1, 0, "{section:?} cell {local}");
                assert_eq!(
                    word >> 8 & 15,
                    seed.emission as u32,
                    "{section:?} cell {local}"
                );
                assert_eq!(word >> 12 & 15, 0, "{section:?} cell {local}");
                assert_eq!(
                    word >> 16,
                    seed.light_type.0 as u32,
                    "{section:?} cell {local}"
                );
                if seed.emission > 0 {
                    emitting += 1;
                    let at = [local & 15, local >> 8, local >> 4 & 15].map(|c| c as u8);
                    let (min, max) = boxes
                        .entry((seed.light_type, seed.emission))
                        .or_insert((at, at));
                    for axis in 0..3 {
                        min[axis] = min[axis].min(at[axis]);
                        max[axis] = max[axis].max(at[axis]);
                    }
                }
            }
            let want: Vec<Emitter> = boxes
                .into_iter()
                .map(|((light_type, level), (min, max))| Emitter {
                    light_type,
                    level,
                    min,
                    max,
                })
                .collect();
            assert_eq!(packed.emitters, want, "{section:?}");
        }
    }
    assert!(emitting > 0);
}

mod exhaustive {
    use super::*;

    proptest! {
        #[test]
        fn every_brick_matches_the_region_edge_costs_on_random_regions(parts in world_parts()) {
            let world = random_world(CENTRE, parts);
            let found = brick_mismatch(CENTRE, world.bounds, &registry(), &colours(), world.cells());
            prop_assert_eq!(found, None);
        }

        #[test]
        fn the_reach_palette_keeps_every_type_that_reaches_the_output(parts in world_parts()) {
            let world = random_world(CENTRE, parts);
            let (registry, colours) = (registry(), colours());
            let packed: HashMap<SectionPos, PackedBrick> = neighbours(CENTRE)
                .map(|s| (s, pack(&section_bricks(s, world.bounds, &registry, &colours, world.cells()))))
                .collect();
            let reach = reaching_types(CENTRE, |s| packed.get(&s).map(|b| b.emitters.as_slice()));
            let (min, size) = section_output(CENTRE);
            let region = Region::new(min, size, world.bounds, &registry, world.cells());
            let present = Palette::of(&region, &registry, &colours).types;
            for t in &reach {
                prop_assert!(present.contains(t), "type {} has no emitter in the region", t.0);
            }
            for &t in &present {
                if oracle(&world, t).iter().any(|&level| level > 0) {
                    prop_assert!(reach.contains(&t), "type {} lights the output", t.0);
                }
            }
        }
    }
}
