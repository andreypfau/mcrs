mod common;

use common::*;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light::level::LightBounds;
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::propagate::{Lanes, colour_section};
use mcrs_minecraft_light_color::region::{Palette, Region, lane_bytes};
use mcrs_minecraft_light_color::resolve::{light_weight, resolve};
use proptest::prelude::*;

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

fn many_types(count: u16) -> Neighbourhood {
    let mut world = Neighbourhood::air(CENTRE);
    let base = CENTRE.y * 16;
    for x in -16..32 {
        for z in -16..32 {
            world.set(BlockPos::new(x, base + 3, z), STONE);
        }
        world.set(BlockPos::new(x, base + 8, 6), WATER);
        world.set(BlockPos::new(x, base + 8, 7), LEAVES);
    }
    for i in 0..count {
        let (x, z) = ((i % 7) as i32 * 2 + 1, (i / 7) as i32 * 2 + 1);
        world.set(
            BlockPos::new(x, base + 4 + (i % 3) as i32, z),
            palette_block(i),
        );
    }
    world
}

#[test]
fn sixteen_seventeen_and_forty_types_each_match_the_server_rule() {
    for (count, bytes) in [(16, 16), (17, 24), (40, 40)] {
        let world = many_types(count);
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
    let mut world = Neighbourhood::air(CENTRE);
    let glowstone = BlockPos::new(8, CENTRE.y * 16 + 8, 8);
    world.set(glowstone, GLOWSTONE);
    for dir in mcrs_minecraft_core::Direction::all() {
        world.set(glowstone + dir.normal(), TORCH);
    }
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

fn soul_and_lava_by_a_wall() -> Neighbourhood {
    let mut world = Neighbourhood::air(CENTRE);
    let base = CENTRE.y * 16;
    for y in base..base + 16 {
        for z in -16..32 {
            world.set(BlockPos::new(7, y, z), STONE);
        }
    }
    world.set(BlockPos::new(7, base + 5, 5), BOTTOM_SLAB);
    world.set(BlockPos::new(3, base + 5, 5), SOUL);
    world.set(BlockPos::new(12, base + 2, 9), LAVA);
    world
}

#[test]
fn nothing_is_retained_between_recomputes() {
    let (a, b) = (torch_on_a_floor(), soul_and_lava_by_a_wall());
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
    let mut world = torch_on_a_floor();
    let unknown = VoxelId(u16::MAX - 1);
    let torch = BlockPos::new(5, CENTRE.y * 16 + 3, 5);
    let beside = BlockPos::new(4, torch.y, 5);
    world.set(beside, unknown);
    world.set(BlockPos::new(5, torch.y + 1, 5), unknown);
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

fn placed_block() -> impl Strategy<Value = VoxelId> {
    let weighted = [
        (AIR, 10),
        (STONE, 10),
        (GLASS, 10),
        (WATER, 10),
        (LEAVES, 10),
        (BOTTOM_SLAB, 10),
        (TOP_SLAB, 10),
        (STAIRS, 10),
        (GLOWSTONE, 1),
        (TORCH, 1),
        (SOUL, 1),
        (REDSTONE, 1),
        (LAVA, 1),
    ];
    let pool: Vec<VoxelId> = weighted
        .iter()
        .flat_map(|&(block, weight)| std::iter::repeat_n(block, weight))
        .collect();
    prop::sample::select(pool)
}

fn base_fill(kind: u8) -> Option<VoxelId> {
    [Some(AIR), Some(STONE), Some(WATER), Some(LEAVES), None][kind as usize]
}

fn world_bounds(place: u8) -> LightBounds {
    match place {
        0 => LightBounds::new(CENTRE.y, CENTRE.y + 6),
        1 => LightBounds::new(CENTRE.y - 6, CENTRE.y),
        _ => LightBounds::new(CENTRE.y - 6, CENTRE.y + 6),
    }
}

proptest! {
    #[test]
    fn every_lane_matches_the_server_rule_on_random_regions(
        place in 0..3u8,
        centre_fill in 0..4u8,
        fills in prop::collection::vec(0..5u8, 26),
        placements in prop::collection::vec((0..48i32, 0..48i32, 0..48i32, placed_block()), 0..=1500),
    ) {
        let mut kinds = fills;
        kinds.insert(13, centre_fill);
        let mut world = Neighbourhood::filled(CENTRE, world_bounds(place), |pos| {
            let d = pos.0 - CENTRE.0 + 1;
            base_fill(kinds[(d.x + 3 * d.z + 9 * d.y) as usize])
        });
        let min = world.block_min();
        for (x, y, z, block) in placements {
            world.set(BlockPos::new(min.x + x, min.y + y, min.z + z), block);
        }
        let computed = compute(&world);
        for &t in &computed.palette.types {
            prop_assert_eq!(lane_mismatch(&world, &computed, t), None);
        }
    }
}
