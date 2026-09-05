use bevy_ecs::prelude::Entity;
mod common;

use std::sync::Arc;

use common::*;

fn combined(world: &LightWorld, pos: BlockPos, sky_darken: u8) -> u8 {
    LightLevel::brightness(
        world.light_at(pos, Layer::Block),
        world.light_at(pos, Layer::Sky),
        sky_darken,
    )
    .get()
}
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_math::{BlockPos, ChunkPos};
use mcrs_voxel_storage::VoxelId;

#[test]
fn block_light_dims_by_one_per_step() {
    let mut world = TestWorld::new(2, 2, 2);
    world.set(BlockPos::new(8, 8, 8), TORCH);

    assert_eq!(world.block_light(BlockPos::new(8, 8, 8)), 14);
    assert_eq!(world.block_light(BlockPos::new(9, 8, 8)), 13);
    assert_eq!(world.block_light(BlockPos::new(10, 8, 8)), 12);
    // Diagonals are two steps away: light only crosses faces.
    assert_eq!(world.block_light(BlockPos::new(9, 9, 8)), 12);
    world.check_against_reference();
}

/// The trap in the sky source rule: glass and water both let light through, and
/// both cost the same to enter, but only water ends the run of sky sources.
#[test]
fn glass_keeps_the_sky_column_but_water_does_not() {
    let mut world = TestWorld::new(2, 3, 2);
    let (x, z) = (8, 8);
    world.build_shaft(x, z, 44, 25);

    world.set(BlockPos::new(x, 40, z), GLASS);
    assert_eq!(
        world.sky_profile(x, z, 40, 34),
        vec![15, 15, 15, 15, 15, 15, 15],
        "glass has dampening 0, so the column of sources continues through it"
    );
    world.check_against_reference();

    world.set(BlockPos::new(x, 40, z), WATER);
    assert_eq!(
        world.sky_profile(x, z, 40, 34),
        vec![14, 13, 12, 11, 10, 9, 8],
        "water has dampening 1, which ends the column and then costs one per step"
    );
    world.check_against_reference();
}

#[test]
fn opaque_and_shaped_blocks_both_end_the_sky_column() {
    let mut world = TestWorld::new(2, 3, 2);
    let (x, z) = (8, 8);
    world.build_shaft(x, z, 44, 25);

    world.set(BlockPos::new(x, 40, z), TINTED_GLASS);
    assert_eq!(
        world.sky_light(BlockPos::new(x, 40, z)),
        0,
        "dampening 15 stops it"
    );
    assert_eq!(world.sky_light(BlockPos::new(x, 39, z)), 0);

    world.set(BlockPos::new(x, 40, z), TOP_SLAB);
    assert_eq!(
        world.sky_light(BlockPos::new(x, 40, z)),
        0,
        "a top slab has dampening 0 and stops the column purely through its shape"
    );
    assert_eq!(world.sky_light(BlockPos::new(x, 39, z)), 0);
    world.check_against_reference();
}

#[test]
fn a_bottom_slab_is_lit_but_shadows_what_is_under_it() {
    let mut world = TestWorld::new(2, 3, 2);
    let (x, z) = (8, 8);
    world.build_shaft(x, z, 44, 25);
    world.set(BlockPos::new(x, 40, z), BOTTOM_SLAB);

    assert_eq!(
        world.sky_light(BlockPos::new(x, 40, z)),
        15,
        "its upper face is open, so the slab itself is still a source"
    );
    assert_eq!(
        world.sky_light(BlockPos::new(x, 39, z)),
        0,
        "its lower face is solid, so nothing passes downward"
    );
    world.check_against_reference();
}

/// Occlusion is a property of the pair of facing shapes, not of either block.
#[test]
fn two_half_faces_occlude_together_but_not_separately() {
    let mut world = TestWorld::new(3, 2, 2);
    let y = 8;
    let z = 8;

    // A one-block tunnel through stone, so that light has no way around the seam.
    let mut walls = Vec::new();
    for x in 0..48 {
        for wy in (y - 1)..=(y + 1) {
            for wz in (z - 1)..=(z + 1) {
                if (wy, wz) != (y, z) {
                    walls.push((BlockPos::new(x, wy, wz), STONE));
                }
            }
        }
    }
    world.set_many(walls);
    world.set(BlockPos::new(4, y, z), TORCH);

    let open: Vec<u8> = (6..12)
        .map(|x| world.block_light(BlockPos::new(x, y, z)))
        .collect();
    assert_eq!(open, vec![12, 11, 10, 9, 8, 7]);

    world.set_many([
        (BlockPos::new(8, y, z), BOTTOM_SLAB),
        (BlockPos::new(9, y, z), BOTTOM_SLAB),
    ]);
    let two_bottoms: Vec<u8> = (6..12)
        .map(|x| world.block_light(BlockPos::new(x, y, z)))
        .collect();
    assert_eq!(
        two_bottoms, open,
        "two lower halves leave the upper half of the seam open"
    );
    world.check_against_reference();

    world.set(BlockPos::new(9, y, z), TOP_SLAB);
    let mixed: Vec<u8> = (6..12)
        .map(|x| world.block_light(BlockPos::new(x, y, z)))
        .collect();
    assert_eq!(
        mixed,
        vec![12, 11, 10, 0, 0, 0],
        "the lower slab is still lit; the seam past it is covered by the two halves together"
    );
    world.check_against_reference();
}

#[test]
fn removing_an_emitter_removes_its_light() {
    let mut world = TestWorld::new(3, 3, 3);
    let pos = BlockPos::new(24, 24, 24);
    world.set(pos, GLOWSTONE);
    assert_eq!(world.block_light(BlockPos::new(30, 24, 24)), 9);

    world.set(pos, AIR);
    assert_eq!(world.block_light(pos), 0);
    assert_eq!(world.block_light(BlockPos::new(30, 24, 24)), 0);
    world.check_against_reference();
}

/// The case that forces a hand-written decrease pass to carry a "restore this
/// source" flag: a dim emitter standing in a bright neighbour's light, which
/// must survive that neighbour being removed. Erasing and refilling gets it for
/// free, because the emitter is re-seeded from the block itself.
#[test]
fn a_dim_emitter_survives_its_bright_neighbour_disappearing() {
    let mut world = TestWorld::new(3, 3, 3);
    let bright = BlockPos::new(24, 24, 24);
    let dim = BlockPos::new(26, 24, 24);

    world.set(bright, GLOWSTONE);
    world.set(dim, TORCH);
    assert_eq!(
        world.block_light(dim),
        14,
        "the brighter neighbour dominates"
    );

    world.set(bright, AIR);
    assert_eq!(world.block_light(dim), 14, "the torch is still a source");
    assert_eq!(world.block_light(BlockPos::new(27, 24, 24)), 13);
    world.check_against_reference();
}

/// Sealing a shaft has to clear the lit column that hangs *below* the lowest sky
/// source, not just the run of sources itself. Water ends the column at its own
/// level while still passing light downward, so the affected cells extend
/// fifteen further down than the sources do.
#[test]
fn sealing_a_shaft_clears_the_lit_column_under_the_water() {
    let mut world = TestWorld::new(3, 6, 3);
    let (x, z) = (24, 24);

    // A stone ceiling with a single hole, and water part way down the shaft.
    let mut ceiling: Vec<(BlockPos, VoxelId)> = Vec::new();
    for cz in 0..48 {
        for cx in 0..48 {
            if (cx, cz) != (x, z) {
                ceiling.push((BlockPos::new(cx, 90, cz), STONE));
            }
        }
    }
    world.set_many(ceiling);
    world.set(BlockPos::new(x, 50, z), WATER);

    assert_eq!(world.sky_light(BlockPos::new(x, 51, z)), 15);
    assert_eq!(world.sky_light(BlockPos::new(x, 50, z)), 14);
    assert_eq!(world.sky_light(BlockPos::new(x, 45, z)), 9);
    assert_eq!(world.sky_light(BlockPos::new(x, 37, z)), 1);

    world.set(BlockPos::new(x, 90, z), STONE);

    assert_eq!(world.sky_light(BlockPos::new(x, 51, z)), 0);
    assert_eq!(
        world.sky_light(BlockPos::new(x, 45, z)),
        0,
        "cells below the lowest source must be cleared too"
    );
    assert_eq!(world.sky_light(BlockPos::new(x, 37, z)), 0);
    world.check_against_reference();
}

#[test]
fn loading_a_section_lets_light_in_from_its_neighbour() {
    let registry = registry();
    let mut world = LightWorld::new(registry, LightBounds::new(0, 0));

    world.update_now([Edit::LoadSection {
        entity: Entity::PLACEHOLDER,
        pos: ChunkPos::new(0, 0, 0),
        blocks: Arc::new(filled(AIR)),
    }]);
    world.update_now([Edit::SetBlock {
        pos: BlockPos::new(15, 8, 8),
        block: GLOWSTONE,
    }]);
    assert_eq!(
        world.light_at(BlockPos::new(15, 8, 8), Layer::Block).get(),
        15
    );

    world.update_now([Edit::LoadSection {
        entity: Entity::PLACEHOLDER,
        pos: ChunkPos::new(1, 0, 0),
        blocks: Arc::new(filled(AIR)),
    }]);
    assert_eq!(
        world.light_at(BlockPos::new(16, 8, 8), Layer::Block).get(),
        14,
        "a section that has just appeared must be lit by what already surrounds it"
    );
    assert_eq!(
        world.light_at(BlockPos::new(20, 8, 8), Layer::Block).get(),
        10
    );
}

#[test]
fn unloading_a_section_takes_its_light_with_it() {
    let registry = registry();
    let mut world = LightWorld::new(registry, LightBounds::new(0, 0));
    for x in 0..2 {
        world.update_now([Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(x, 0, 0),
            blocks: Arc::new(filled(AIR)),
        }]);
    }
    world.update_now([Edit::SetBlock {
        pos: BlockPos::new(15, 8, 8),
        block: GLOWSTONE,
    }]);
    assert_eq!(
        world.light_at(BlockPos::new(18, 8, 8), Layer::Block).get(),
        12
    );

    world.update_now([Edit::UnloadSection {
        pos: ChunkPos::new(0, 0, 0),
    }]);
    assert_eq!(
        world.light_at(BlockPos::new(18, 8, 8), Layer::Block).get(),
        0,
        "the emitter is gone, and unloaded space is opaque rather than empty"
    );
}

#[test]
fn brightness_applies_sky_darkening_at_read_time() {
    let mut world = TestWorld::new(2, 2, 2);
    let pos = BlockPos::new(8, 20, 8);
    assert_eq!(world.sky_light(pos), 15);

    assert_eq!(combined(&world.world, pos, 0), 15, "noon");
    assert_eq!(combined(&world.world, pos, 5), 10, "thunderstorm");
    assert_eq!(combined(&world.world, pos, 11), 4, "midnight");

    world.set(BlockPos::new(8, 19, 8), TORCH);
    assert_eq!(
        combined(&world.world, BlockPos::new(8, 18, 8), 11),
        13,
        "block light is never darkened"
    );
}

#[test]
fn a_batch_of_edits_agrees_with_the_reference() {
    let mut world = TestWorld::new(3, 4, 3);

    // A terrain-like slab of stone with a cave under it, a glass skylight, and
    // some emitters scattered below.
    let mut terrain = Vec::new();
    for z in 0..48 {
        for x in 0..48 {
            terrain.push((BlockPos::new(x, 40, z), STONE));
        }
    }
    world.set_many(terrain);
    world.set_many([
        (BlockPos::new(10, 40, 10), GLASS),
        (BlockPos::new(11, 40, 10), GLASS),
        (BlockPos::new(30, 40, 30), AIR),
        (BlockPos::new(20, 30, 20), TORCH),
        (BlockPos::new(35, 25, 12), GLOWSTONE),
        (BlockPos::new(5, 20, 40), TORCH),
        (BlockPos::new(24, 36, 24), BOTTOM_SLAB),
        (BlockPos::new(25, 36, 24), TOP_SLAB),
        (BlockPos::new(40, 38, 8), WATER),
    ]);
    world.check_against_reference();

    // Now break some of it again.
    world.set_many([
        (BlockPos::new(20, 30, 20), AIR),
        (BlockPos::new(10, 40, 10), STONE),
        (BlockPos::new(30, 40, 30), STONE),
        (BlockPos::new(12, 40, 12), AIR),
    ]);
    world.check_against_reference();
}

#[test]
fn the_frontier_never_needs_more_than_fifteen_rounds() {
    let mut world = TestWorld::new(3, 4, 3);
    let mut worst = 0;

    let mut record = |stats: EpochStats| {
        worst = worst.max(stats.block_rounds).max(stats.sky_rounds);
    };

    let mut terrain = Vec::new();
    for z in 0..48 {
        for x in 0..48 {
            terrain.push((BlockPos::new(x, 40, z), STONE));
        }
    }
    record(world.set_many(terrain));
    record(world.set(BlockPos::new(24, 40, 24), AIR));
    record(world.set(BlockPos::new(24, 20, 24), GLOWSTONE));
    record(world.set(BlockPos::new(24, 20, 24), AIR));
    record(world.set(BlockPos::new(24, 40, 24), STONE));

    assert!(worst <= 15, "expected at most 15 rounds, saw {worst}");
    assert!(worst > 0, "the test did no work at all");
}

#[test]
fn the_result_does_not_depend_on_the_thread_schedule() {
    fn build() -> TestWorld {
        let mut world = TestWorld::new(3, 3, 3);
        let mut terrain = Vec::new();
        for z in 0..48 {
            for x in 0..48 {
                if (x * 7 + z * 13) % 5 != 0 {
                    terrain.push((BlockPos::new(x, 30, z), STONE));
                }
            }
        }
        world.set_many(terrain);
        world.set_many([
            (BlockPos::new(8, 20, 8), GLOWSTONE),
            (BlockPos::new(33, 25, 41), TORCH),
            (BlockPos::new(20, 30, 20), WATER),
        ]);
        world
    }

    let first = build();
    for _ in 0..4 {
        let again = build();
        for y in 0..48 {
            for z in 0..48 {
                for x in 0..48 {
                    let pos = BlockPos::new(x, y, z);
                    assert_eq!(
                        first.block_light(pos),
                        again.block_light(pos),
                        "block at {pos:?}"
                    );
                    assert_eq!(first.sky_light(pos), again.sky_light(pos), "sky at {pos:?}");
                }
            }
        }
    }
}

#[test]
fn an_edit_that_changes_nothing_costs_nothing() {
    let mut world = TestWorld::new(2, 2, 2);
    world.set(BlockPos::new(8, 8, 8), STONE);
    let stats = world.set(BlockPos::new(8, 8, 8), STONE);
    assert_eq!(stats, EpochStats::default(), "no epoch should have run");
}

#[test]
fn a_shaped_state_agrees_with_the_reference_solver() {
    let mut world = TestWorld::new(2, 3, 2);
    let (x, z) = (8, 8);
    world.build_shaft(x, z, 44, 25);
    world.set(BlockPos::new(x, 40, z), TOP_SLAB);
    world.check_against_reference();
}

/// An asymmetric scene: a permuted step axis leaks light the wrong way here,
/// which the reference solver sees even though an open world would hide it.
#[test]
fn an_obstructed_scene_matches_the_reference_solver() {
    let mut world = TestWorld::new(2, 2, 2);

    let mut walls = Vec::new();
    for y in 4..12 {
        for z in 0..10 {
            walls.push((BlockPos::new(12, y, z), STONE));
        }
    }
    for x in 0..20 {
        walls.push((BlockPos::new(x, 6, 14), WATER));
    }
    world.set_many(walls);

    world.set(BlockPos::new(9, 7, 3), TORCH);
    world.set(BlockPos::new(20, 9, 21), TORCH);
    world.check_against_reference();

    world.set(BlockPos::new(12, 7, 5), AIR);
    world.check_against_reference();
}

/// A section of one repeated block answers the sky scan without reading its
/// blocks, so the seam entering it and the seam inside it both have to be
/// tested — glass keeps the column across both, water ends it at the first.
#[test]
fn uniform_sections_agree_with_the_reference_solver() {
    let mut world = LightWorld::new(registry(), LightBounds::new(0, 2));
    let loads: Vec<Edit> = (0..3)
        .map(|y| Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(0, y, 0),
            blocks: Arc::new(filled(match y {
                2 => GLASS,
                1 => WATER,
                _ => AIR,
            })),
        })
        .collect();
    world.update_now(loads);

    assert_eq!(
        world.light_at(BlockPos::new(8, 47, 8), Layer::Sky).get(),
        15
    );
    assert_eq!(
        world.light_at(BlockPos::new(8, 32, 8), Layer::Sky).get(),
        15
    );
    assert_eq!(
        world.light_at(BlockPos::new(8, 31, 8), Layer::Sky).get(),
        14
    );

    let reference = Reference::compute(&world, BlockPos::new(0, 0, 0), BlockPos::new(15, 47, 15));
    if let Some(diff) = reference.diff(&world) {
        panic!("engine disagrees with the reference solver: {diff}");
    }
}

#[test]
fn a_section_the_epoch_recomputed_but_did_not_change_keeps_its_buffer() {
    let mut world = TestWorld::new(3, 1, 1);
    world.set(BlockPos::new(8, 8, 8), TORCH);
    let lit = ChunkPos::new(0, 0, 0);
    let before = world.world.section(lit).unwrap().block_light.clone();

    let stats = world.set(BlockPos::new(24, 8, 8), STONE);
    assert!(
        stats.area_cells > 16 * 16 * 16,
        "the second edit has to reach past its own section for this to say anything"
    );

    let after = &world.world.section(lit).unwrap().block_light;
    match (&before, after) {
        (LightStorage::Dense(before), LightStorage::Dense(after)) => assert!(
            Arc::ptr_eq(before, after),
            "unchanged light was recomputed into a fresh buffer"
        ),
        (before, after) => panic!("expected dense light on both sides, got {before:?} / {after:?}"),
    }
}
