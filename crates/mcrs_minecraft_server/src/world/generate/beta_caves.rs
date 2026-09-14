use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_decoration::carver::mask::CarvingMask;
use mcrs_minecraft_decoration::carver::water::WaterMask;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::NoiseRouter;

use crate::world::generate::ColumnBlocks;
use crate::world::generate::modern_carvers::{CarverBiomeTable, carve_sources};

pub struct BetaCaveBlockIds {
    pub air: VoxelId,
    pub lava: VoxelId,
    pub stone: VoxelId,
    pub dirt: VoxelId,
    pub grass: VoxelId,
    /// Beta's abort tests both flowing and stationary water; the fill and the
    /// surface place only the one source state.
    pub water: VoxelId,
}

impl BetaCaveBlockIds {
    pub fn resolve(blocks: &BlockDefinitions) -> Self {
        let state = |name: &str| -> VoxelId { blocks.default_state(name).into() };
        BetaCaveBlockIds {
            air: state("minecraft:air"),
            lava: state("minecraft:lava"),
            stone: state("minecraft:stone"),
            dirt: state("minecraft:dirt"),
            grass: state("minecraft:grass_block"),
            water: state("minecraft:water"),
        }
    }
}

/// Every water block of the column, as the carver's abort oracle.
///
/// Section cells are dense and in palette index order, so one linear scan per
/// section covers the whole column.
fn water_mask(column: &ColumnBlocks, water: VoxelId) -> WaterMask {
    let mut mask = WaterMask::default();
    for (slot, &section_y) in column.y_sections().iter().enumerate() {
        let base_y = section_y * 16;
        if base_y >= 128 || base_y + 16 <= 0 {
            continue;
        }
        for (index, cell) in column.section_cells(slot).iter().enumerate() {
            if cell.get() != water {
                continue;
            }
            mask.insert(
                (index % 16) as i32,
                base_y + (index / 256) as i32,
                ((index / 16) % 16) as i32,
            );
        }
    }
    mask
}

/// Below this the freed block is lava rather than air.
const LAVA_LEVEL: i32 = 10;

/// Fill the space the carver freed: lava under the lava level, air above it,
/// and dirt turned to grass directly under the first grass seen coming down.
///
/// One pass per carved run, top down, which is where the grass fixup has to
/// look: the block it converts is the next Y the same run visits.
fn apply_cave_substance(column: &ColumnBlocks, mask: &CarvingMask, ids: &BetaCaveBlockIds) {
    mask.visit(|x, z, bottom_y, top_y| {
        let mut has_grass = false;
        for y in (bottom_y..=top_y).rev() {
            let state = column.get(x, y, z).unwrap_or(ids.air);
            if state == ids.grass {
                has_grass = true;
            }
            if state != ids.stone && state != ids.dirt && state != ids.grass {
                continue;
            }
            // Beta's lava threshold is on the Y the ellipsoid test accepted,
            // which is one below the block that test frees.
            if y - 1 < LAVA_LEVEL {
                column.set(x, y, z, ids.lava);
            } else {
                column.set(x, y, z, ids.air);
                if has_grass && column.get(x, y - 1, z) == Some(ids.dirt) {
                    column.set(x, y - 1, z, ids.grass);
                }
            }
        }
    });
}

/// Beta's carving of one column: the source loop every dimension shares, with
/// Beta's carver in it, then Beta's own fill of what it freed.
#[allow(clippy::too_many_arguments)]
pub fn apply_beta_carvers(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    router: &NoiseRouter,
    ws: &mut Workspace,
    biomes: &CarverBiomeTable,
    height: HeightContext,
    ids: &BetaCaveBlockIds,
) {
    let mask = carve_sources(
        || water_mask(column, ids.water),
        chunk_x,
        chunk_z,
        world_seed,
        router,
        ws,
        biomes,
        height,
    );
    apply_cave_substance(column, &mask, ids);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generate::tests::corpus;

    #[test]
    fn the_fill_frees_only_what_beta_carves_and_floors_it_with_lava() {
        let ids = BetaCaveBlockIds::resolve(corpus());
        let sand: VoxelId = corpus().default_state("minecraft:sand").into();
        let sections: Vec<i32> = (0..8).collect();
        let column = ColumnBlocks::new(&sections);
        let mut mask = CarvingMask::new(16, 1, 120);
        for (x, y, state) in [
            (0, 10, ids.stone),
            (0, 11, ids.stone),
            (1, 50, sand),
            (2, 51, ids.grass),
            (2, 50, ids.dirt),
        ] {
            column.set(x, y, 0, state);
            mask.carve(x, y, 0);
        }
        column.set(2, 49, 0, ids.dirt);

        apply_cave_substance(&column, &mask, &ids);

        assert_eq!(column.get(0, 10, 0), Some(ids.lava), "tested at Y 9");
        assert_eq!(column.get(0, 11, 0), Some(ids.air), "tested at Y 10");
        assert_eq!(column.get(1, 50, 0), Some(sand), "Beta carves no sand");
        assert_eq!(column.get(2, 51, 0), Some(ids.air));
        assert_eq!(column.get(2, 50, 0), Some(ids.air));
        assert_eq!(
            column.get(2, 49, 0),
            Some(ids.grass),
            "the dirt under a carved lawn turns to grass"
        );
    }
}
