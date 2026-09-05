//! Columns reach the server one at a time, so a column is lit while its
//! neighbour is still absent and therefore opaque. These pin that the seam is
//! repaired when the neighbour does arrive.

mod common;

use std::sync::Arc;

use common::{AIR, Reference, STONE, WATER, filled, registry};
use bevy_ecs::prelude::Entity;
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_math::{BlockPos, ChunkPos};
use mcrs_voxel_storage::VoxelId;

const SECTIONS_Y: i32 = 4;

fn world() -> LightWorld {
    LightWorld::new(registry(), LightBounds::new(0, SECTIONS_Y - 1))
}

/// One column of sections, `floor_y` filled with `floor` and the rest air.
fn load_column(world: &mut LightWorld, x: i32, z: i32, floor_y: i32, floor: VoxelId) {
    let loads: Vec<Edit> = (0..SECTIONS_Y)
        .map(|y| Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(x, y, z),
            blocks: Arc::new(filled(if y == floor_y { floor } else { AIR })),
        })
        .collect();
    world.update_now(loads);
}

fn check(world: &LightWorld, columns_x: i32, columns_z: i32) {
    let min = BlockPos::new(0, 0, 0);
    let max = BlockPos::new(columns_x * 16 - 1, SECTIONS_Y * 16 - 1, columns_z * 16 - 1);
    let reference = Reference::compute(world, min, max);
    if let Some(diff) = reference.diff(world) {
        panic!("engine disagrees with the reference solver: {diff}");
    }
}

#[test]
fn a_column_lit_beside_an_absent_neighbour_is_repaired_when_it_arrives() {
    let mut world = world();
    load_column(&mut world, 0, 0, 0, STONE);
    load_column(&mut world, 1, 0, 0, STONE);
    check(&world, 2, 1);
}

#[test]
fn a_row_of_columns_arriving_one_at_a_time_matches_one_pass() {
    let mut world = world();
    for x in 0..4 {
        load_column(&mut world, x, 0, 0, STONE);
    }
    check(&world, 4, 1);
}

#[test]
fn a_block_of_columns_arriving_one_at_a_time_matches_one_pass() {
    let mut world = world();
    for x in 0..3 {
        for z in 0..3 {
            load_column(&mut world, x, z, 0, STONE);
        }
    }
    check(&world, 3, 3);
}

/// Water ends the sky column but still passes light, so a seam here shows up as
/// a band of the surface being darker than the column beside it.
#[test]
fn water_columns_arriving_one_at_a_time_match_one_pass() {
    let mut world = world();
    for x in 0..3 {
        for z in 0..3 {
            load_column(&mut world, x, z, 1, WATER);
        }
    }
    check(&world, 3, 3);
}

#[test]
fn columns_arriving_in_reverse_order_match_one_pass() {
    let mut world = world();
    for x in (0..3).rev() {
        load_column(&mut world, x, 0, 0, STONE);
    }
    check(&world, 3, 1);
}

/// `process_completed_columns` inserts a column's sections as their tasks
/// finish, so a column can enter the light world in pieces. The sky floor of
/// the whole column moves when the top arrives, and every cell below it is a
/// source that has to be relit — not just the fifteen under the new section.
#[test]
fn a_column_arriving_bottom_half_first_matches_one_pass() {
    let mut world = world();
    let half = SECTIONS_Y / 2;
    let lower: Vec<Edit> = (0..half)
        .map(|y| Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(0, y, 0),
            blocks: Arc::new(filled(AIR)),
        })
        .collect();
    world.update_now(lower);
    let upper: Vec<Edit> = (half..SECTIONS_Y)
        .map(|y| Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(0, y, 0),
            blocks: Arc::new(filled(AIR)),
        })
        .collect();
    world.update_now(upper);
    check(&world, 1, 1);
}

#[test]
fn a_column_arriving_one_section_at_a_time_bottom_up_matches_one_pass() {
    let mut world = world();
    for y in 0..SECTIONS_Y {
        world.update_now([Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(0, y, 0),
            blocks: Arc::new(filled(AIR)),
        }]);
    }
    check(&world, 1, 1);
}

/// Sea level across whole chunks: water below, open air above. The surface is
/// flat and unbroken, so every cell of it sees the same sky, and a chunk
/// boundary is not a thing light knows about.
fn load_ocean_column(world: &mut LightWorld, x: i32, z: i32, water_sections: i32) {
    let loads: Vec<Edit> = (0..SECTIONS_Y)
        .map(|y| Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(x, y, z),
            blocks: Arc::new(filled(if y < water_sections {
                WATER
            } else {
                AIR
            })),
        })
        .collect();
    world.update_now(loads);
}

#[test]
fn an_ocean_surface_is_lit_the_same_everywhere_including_chunk_borders() {
    const WATER_SECTIONS: i32 = 2;
    const COLUMNS: i32 = 3;
    let surface = WATER_SECTIONS * 16 - 1;

    let mut world = world();
    for x in 0..COLUMNS {
        for z in 0..COLUMNS {
            load_ocean_column(&mut world, x, z, WATER_SECTIONS);
        }
    }

    let air_above = |w: &LightWorld, x: i32, z: i32| {
        w.light_at(BlockPos::new(x, surface + 1, z), Layer::Sky)
            .get()
    };
    let top_of_water =
        |w: &LightWorld, x: i32, z: i32| w.light_at(BlockPos::new(x, surface, z), Layer::Sky).get();

    let width = COLUMNS * 16;
    let expected_air = air_above(&world, width / 2, width / 2);
    let expected_water = top_of_water(&world, width / 2, width / 2);
    assert_eq!(expected_air, 15, "open air over the sea is full sky light");

    for x in 0..width {
        for z in 0..width {
            assert_eq!(
                air_above(&world, x, z),
                expected_air,
                "air over the sea at ({x}, {z}), {} blocks from a chunk border",
                (x % 16).min(15 - x % 16).min((z % 16).min(15 - z % 16)),
            );
            assert_eq!(
                top_of_water(&world, x, z),
                expected_water,
                "sea surface at ({x}, {z}), {} blocks from a chunk border",
                (x % 16).min(15 - x % 16).min((z % 16).min(15 - z % 16)),
            );
        }
    }
}
