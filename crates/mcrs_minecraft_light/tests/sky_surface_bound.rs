//! A column's surface is only ever a bound on the sky scan. These pin that it
//! never changes an answer, and that it is dropped the moment the light world's
//! own copy of the column moves out from under it.

mod common;

use std::sync::Arc;

use bevy_ecs::prelude::Entity;
use common::{AIR, BOTTOM_SLAB, GLASS, LEAVES, Reference, STONE, TOP_SLAB, filled, registry};
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_math::{BlockPos, ChunkPos, ColumnPos};

const SECTIONS_Y: i32 = 5;

fn world() -> LightWorld {
    LightWorld::new(registry(), LightBounds::new(0, SECTIONS_Y - 1))
}

/// Terrain in section 0, a canopy scattered through section 2, air elsewhere:
/// the shape the prologue cannot skip, because the canopy section is neither
/// uniform nor empty.
fn stack() -> Vec<SectionBlocks> {
    let mut sections: Vec<SectionBlocks> = (0..SECTIONS_Y)
        .map(|y| filled(if y == 0 { STONE } else { AIR }))
        .collect();

    let canopy = &mut sections[2];
    for i in 0..8u8 {
        canopy.set_cell(i as usize, 4, i as usize, LEAVES);
        canopy.set_cell((i) as usize, (5) as usize, (i + 1) as usize, GLASS);
        // A top slab directly under a bottom slab seals a seam neither seals
        // alone, so the shortcut has to leave these columns to the full walk.
        canopy.set_cell((i + 8) as usize, (7) as usize, (i) as usize, TOP_SLAB);
        canopy.set_cell((i + 8) as usize, (8) as usize, (i) as usize, BOTTOM_SLAB);
    }
    sections
}

fn load(world: &mut LightWorld, sections: &[SectionBlocks]) {
    let edits: Vec<Edit> = sections
        .iter()
        .enumerate()
        .map(|(y, blocks)| Edit::LoadSection {
            entity: Entity::PLACEHOLDER,
            pos: ChunkPos::new(0, y as i32, 0),
            blocks: Arc::new(blocks.clone()),
        })
        .collect();
    world.update_now(edits);
}

/// One past the topmost non-air Y of each block column, which is what the
/// server's `SurfaceHeightmap` holds.
fn surface_of(sections: &[SectionBlocks]) -> Arc<ColumnSurface> {
    let mut surface = ColumnSurface::new((SECTIONS_Y * 16) as u32, 0);
    for z in 0..BLOCKS::SIZE {
        for x in 0..BLOCKS::SIZE {
            let mut top = 0;
            for y in (0..SECTIONS_Y * 16).rev() {
                let blocks = &sections[(y / 16) as usize];
                if blocks.get_cell(x, (y % 16) as usize, z) != AIR {
                    top = y + 1;
                    break;
                }
            }
            surface.set(x, z, top);
        }
    }
    Arc::new(surface)
}

fn floors(world: &LightWorld) -> Vec<i32> {
    (0..BLOCKS::AREA)
        .map(|cell| {
            world.sky_floor(BlockColumn {
                x: (cell & BLOCKS::MASK) as i32,
                z: (cell >> BLOCKS::BITS) as i32,
            })
        })
        .collect()
}

fn assert_lit_correctly(world: &LightWorld) {
    let min = BlockPos::new(0, 0, 0);
    let max = BlockPos::new(15, SECTIONS_Y * 16 - 1, 15);
    let reference = Reference::compute(world, min, max);
    if let Some(diff) = reference.diff(world) {
        panic!("engine disagrees with the reference solver: {diff}");
    }
}

#[test]
fn a_surface_bound_never_changes_a_sky_floor() {
    let sections = stack();

    let mut unbounded = world();
    load(&mut unbounded, &sections);

    let mut bounded = world();
    load(&mut bounded, &sections);
    bounded.update_now(vec![Edit::SetColumnSurface {
        column: ColumnPos::new(0, 0),
        surface: surface_of(&sections),
    }]);
    // The bound only takes effect on the next scan of the column.
    bounded.update_now(vec![Edit::LoadSection {
        entity: Entity::PLACEHOLDER,
        pos: ChunkPos::new(0, 4, 0),
        blocks: Arc::new(sections[4].clone()),
    }]);
    bounded.update_now(vec![Edit::SetColumnSurface {
        column: ColumnPos::new(0, 0),
        surface: surface_of(&sections),
    }]);
    bounded.update_now(vec![Edit::SetBlock {
        pos: BlockPos::new(15, 0, 15),
        block: STONE,
    }]);

    assert_eq!(floors(&bounded), floors(&unbounded));
    assert_lit_correctly(&bounded);
}

#[test]
fn an_edit_above_the_bound_drops_it() {
    let sections = stack();
    let mut bounded = world();
    load(&mut bounded, &sections);
    bounded.update_now(vec![Edit::SetColumnSurface {
        column: ColumnPos::new(0, 0),
        surface: surface_of(&sections),
    }]);

    // A block far above every column's surface. If the stale bound survived the
    // edit, the scan would skip the section holding it and report open sky.
    let raised = BlockPos::new(3, 70, 3);
    bounded.update_now(vec![Edit::SetBlock {
        pos: raised,
        block: STONE,
    }]);

    let mut unbounded = world();
    load(&mut unbounded, &sections);
    unbounded.update_now(vec![Edit::SetBlock {
        pos: raised,
        block: STONE,
    }]);

    assert_eq!(
        bounded.sky_floor(BlockColumn { x: 3, z: 3 }),
        unbounded.sky_floor(BlockColumn { x: 3, z: 3 }),
    );
    assert_eq!(floors(&bounded), floors(&unbounded));
    assert_lit_correctly(&bounded);
}

#[test]
fn a_section_arriving_drops_the_bound() {
    let sections = stack();
    let mut bounded = world();
    load(&mut bounded, &sections);
    bounded.update_now(vec![Edit::SetColumnSurface {
        column: ColumnPos::new(0, 0),
        surface: surface_of(&sections),
    }]);

    // Section 3 comes back holding terrain the bound says is not there.
    let mut ceiling = filled(AIR);
    for i in 0..16u8 {
        ceiling.set_cell(i as usize, 9 as usize, i as usize, STONE);
    }
    let replacement = Edit::LoadSection {
        entity: Entity::PLACEHOLDER,
        pos: ChunkPos::new(0, 3, 0),
        blocks: Arc::new(ceiling.clone()),
    };
    bounded.update_now(vec![replacement.clone()]);

    let mut unbounded = world();
    load(&mut unbounded, &sections);
    unbounded.update_now(vec![replacement]);

    assert_eq!(floors(&bounded), floors(&unbounded));
    assert_lit_correctly(&bounded);
}

/// The shortcut reads the section's top block to carry into the next seam, so a
/// column whose bound sits exactly at a section boundary must still see it.
#[test]
fn a_bound_on_a_section_boundary_is_exact() {
    let mut sections: Vec<SectionBlocks> = (0..SECTIONS_Y)
        .map(|y| filled(if y == 0 { STONE } else { AIR }))
        .collect();
    sections[1].set_cell(0 as usize, 15 as usize, 0 as usize, STONE);

    let mut bounded = world();
    load(&mut bounded, &sections);
    bounded.update_now(vec![Edit::SetColumnSurface {
        column: ColumnPos::new(0, 0),
        surface: surface_of(&sections),
    }]);
    bounded.update_now(vec![Edit::SetBlock {
        pos: BlockPos::new(15, 0, 15),
        block: STONE,
    }]);

    let mut unbounded = world();
    load(&mut unbounded, &sections);
    unbounded.update_now(vec![Edit::SetBlock {
        pos: BlockPos::new(15, 0, 15),
        block: STONE,
    }]);

    assert_eq!(floors(&bounded), floors(&unbounded));
    assert_eq!(bounded.sky_floor(BlockColumn { x: 0, z: 0 }), 32);
}
