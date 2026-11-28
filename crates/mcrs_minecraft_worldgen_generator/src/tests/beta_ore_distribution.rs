use std::cell::Cell;
use std::rc::Rc;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_volume::BoxVolume;
use mcrs_voxel::BlockPos;

use super::counting_rng::CountingRng;
use crate::{BetaOreBlockIds, place_all_ores};

fn stone_volume(stone: VoxelId) -> BoxVolume {
    let bounds = BoundingBox {
        min: BlockPos::new(-16, 0, -16),
        max: BlockPos::new(31, 127, 31),
    };
    BoxVolume::filled(bounds, super::corpus_state_of(stone)).expect("a small box")
}

fn placed(volume: &BoxVolume, stone: VoxelId) -> Vec<((i32, i32), VoxelId)> {
    volume
        .iter()
        .map(|(at, state)| (at, VoxelId(state.number())))
        .filter(|(_, state)| *state != stone)
        .map(|(at, state)| ((at.x.div_euclid(16), at.z.div_euclid(16)), state))
        .collect()
}

fn populate_seed(chunk_x: i32, chunk_z: i32, world_seed: i64) -> i64 {
    let mut s = LegacyRandom::new(world_seed as u64);
    let i1 = s.next_java_long() / 2 * 2 + 1;
    let j1 = s.next_java_long() / 2 * 2 + 1;
    (chunk_x as i64)
        .wrapping_mul(i1)
        .wrapping_add((chunk_z as i64).wrapping_mul(j1))
        ^ world_seed
}

const ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345: u64 = 3737;

const ORE_BLOCKS_CHUNK_0_0_SEED_12345: (usize, usize) = (964, 2580);

fn drive(seed: i64, ids: &BetaOreBlockIds) -> (BoxVolume, u64) {
    let draws = Rc::new(Cell::new(0u64));
    let mut rng = CountingRng::new(seed as u64, draws.clone());
    let mut volume = stone_volume(ids.stone.number().into());
    place_all_ores(&mut volume, 0, 0, &mut rng, ids);
    (volume, draws.get())
}

#[test]
fn veins_cross_the_column_border_on_the_pinned_draw_count() {
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let (volume, draws) = drive(populate_seed(0, 0, 12345), &ids);
    assert_eq!(draws, ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345);
    let placed = placed(&volume, ids.stone.number().into());

    let own = placed.iter().filter(|(col, _)| *col == (0, 0)).count();
    let crossed = placed.len() - own;

    assert_eq!((own, crossed), ORE_BLOCKS_CHUNK_0_0_SEED_12345);

    for (col, _) in &placed {
        assert!(
            (-1..=1).contains(&col.0) && (-1..=1).contains(&col.1),
            "a write reached column {col:?}"
        );
    }

    for state in [
        ids.dirt,
        ids.gravel,
        ids.coal,
        ids.iron,
        ids.gold,
        ids.redstone,
        ids.diamond,
        ids.lapis,
    ] {
        assert!(
            placed.iter().any(|(_, got)| *got == state.number().into()),
            "no block of {state:?} was placed"
        );
    }
}

#[test]
fn the_region_takes_the_veins_the_box_takes() {
    use mcrs_voxel::ColumnPos;

    use crate::ColumnBlocks;
    use crate::beta_ores::apply_beta_ores_in;
    use crate::stages::ColumnRegion;

    let ctx = super::bare_fill_context(super::build_beta_router());
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let stone = VoxelId(ids.stone.number());
    let center = ColumnPos::new(0, 0);
    let snapshots = super::region_of(center, |col| {
        super::flat_snapshot(
            col,
            &ctx.y_sections,
            |section_y| (0..8).contains(&section_y).then_some(stone),
            None,
        )
    });
    let column = ColumnBlocks::new(&ctx.y_sections);
    column.unpack(&snapshots[4].sections);
    let mut region = ColumnRegion::new(&snapshots, &column, &ctx);
    apply_beta_ores_in(&mut region, center.x, center.z, 12345, &ids);

    let mut counts = (0, 0);
    for (col, delta) in region.finish() {
        if col == center {
            counts.0 += delta.writes.len();
        } else {
            counts.1 += delta.writes.len();
        }
    }
    assert_eq!(counts, ORE_BLOCKS_CHUNK_0_0_SEED_12345);
}

#[test]
fn the_populate_feature_is_the_populate_step() {
    use std::sync::Arc;

    use mcrs_voxel::ColumnPos;

    use crate::ColumnBlocks;
    use crate::beta_ores::apply_beta_ores_in;
    use crate::stages::{ColumnRegion, run_column};

    let router = super::build_beta_router();
    let seed = router.world_seed as i64;
    let (_, registry) = super::beta_surface::build_beta_biome_source();
    let program = Arc::new(super::beta_populate_program(&registry, seed));
    let ctx = super::fill_context_with(router, Some(program));
    let stone: VoxelId = super::corpus()
        .default_state("minecraft:stone")
        .unwrap()
        .number()
        .into();
    let center = ColumnPos::new(3, -2);
    let snapshots = super::region_of(center, |col| {
        super::flat_snapshot(
            col,
            &ctx.y_sections,
            |section_y| (0..8).contains(&section_y).then_some(stone),
            None,
        )
    });

    let writes = |populate: &dyn Fn(&mut ColumnRegion<'_>)| {
        let column = ColumnBlocks::new(&ctx.y_sections);
        column.unpack(&snapshots[4].sections);
        let mut region = ColumnRegion::new(&snapshots, &column, &ctx);
        populate(&mut region);
        region
            .finish()
            .into_iter()
            .map(|(col, delta)| (col, delta.writes))
            .collect::<Vec<_>>()
    };
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let through_program = writes(&|region| run_column(&ctx, region, 0));
    let direct = writes(&|region| apply_beta_ores_in(region, center.x, center.z, seed, &ids));

    assert!(
        direct.iter().map(|(_, writes)| writes.len()).sum::<usize>() > 0,
        "the populate step wrote nothing"
    );
    assert_eq!(through_program, direct);
}
