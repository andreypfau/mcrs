use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, Direction};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume, WorldStates};

use crate::canvas::{ChestStates, PieceCanvas};
use crate::{block_mask, state};

pub const BURIED_TREASURE_LOOT: &str = "minecraft:chests/buried_treasure";

#[derive(Clone, Debug)]
pub struct BuriedTreasureBlocks {
    pub sand: VoxelId,
    pub resting: StateMask,
    /// `isAir` or the water or lava block: what the walk fills in around the chest.
    pub open: StateMask,
    pub chest: ChestStates,
}

impl BuriedTreasureBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world: &WorldStates,
    ) -> Result<Self, FeatureCompileError> {
        let mut open = fixedbitset::FixedBitSet::clone(&world.air_states);
        open.union_with(&world.water_states);
        open.union_with(&world.lava_states);
        Ok(BuriedTreasureBlocks {
            sand: state(blocks, "minecraft:sand", &[])?,
            resting: block_mask(
                blocks,
                &[
                    "minecraft:sandstone",
                    "minecraft:stone",
                    "minecraft:andesite",
                    "minecraft:granite",
                    "minecraft:diorite",
                ],
            )?,
            open: open.into(),
            chest: ChestStates::compile(blocks)?,
        })
    }
}

/// `BuriedTreasurePiece.postProcess`: down the piece's column from the ocean
/// floor to the first block resting on stone or sandstone, its open neighbours
/// filled, then the chest. The neighbour writes are not clipped, as the
/// reference's are not; they never leave the piece's column.
pub fn paint_buried_treasure<W: WorldGenVolume, R: Random>(
    b: &BuriedTreasureBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    let (x, z) = (c.bounds.min.x, c.bounds.min.z);
    let floor = c.volume.extent().min_y;
    let mut pos = BlockPos::new(x, c.volume.height(HeightmapName::OceanFloorWg, x, z), z);
    while pos.y > floor {
        let below = c.volume.get(pos + IVec3::NEG_Y);
        if b.resting.contains(below.0 as usize) {
            let current = c.volume.get(pos);
            let soft = if b.open.contains(current.0 as usize) {
                b.sand
            } else {
                current
            };
            for direction in Direction::all() {
                let relative = pos + direction.normal();
                if !c.volume.holds(&b.open, relative) {
                    continue;
                }
                let open_below = c.volume.holds(&b.open, relative + IVec3::NEG_Y);
                let fill = if open_below && direction != Direction::Up {
                    below
                } else {
                    soft
                };
                c.volume.set(relative, fill);
            }
            c.create_chest(rng, &b.chest, pos.x, pos.y, pos.z, BURIED_TREASURE_LOOT);
            return;
        }
        pos.y -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_chunk::BlocksMut;
    use mcrs_minecraft_core::BoundingBox;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_worldgen_feature::placer::{BoxRegion, mask_of};
    use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;

    const AIR: VoxelId = VoxelId(0);
    const STONE: VoxelId = VoxelId(1);
    const WATER: VoxelId = VoxelId(2);
    const GRAVEL: VoxelId = VoxelId(3);
    const SAND: VoxelId = VoxelId(4);
    const CHEST: VoxelId = VoxelId(5);

    fn blocks() -> BuriedTreasureBlocks {
        BuriedTreasureBlocks {
            sand: SAND,
            resting: mask_of([STONE]),
            open: mask_of([AIR, WATER]),
            chest: ChestStates {
                facing: [CHEST; 4],
                states: mask_of([CHEST]),
            },
        }
    }

    fn region() -> BoxRegion {
        let mut region = BoxRegion::new(BlockPos::new(0, 0, 0), BlockPos::new(15, 31, 15), AIR)
            .floor(4, STONE)
            .with_height(|_, _, _, _| 12);
        for y in 5..=7 {
            region.blocks.fill_layer(y, GRAVEL);
        }
        for y in 8..=11 {
            region.blocks.fill_layer(y, WATER);
        }
        region.world.air_states = mask_of([AIR]);
        region.world.solid_render = mask_of([STONE, GRAVEL]);
        region
    }

    #[test]
    fn the_walk_rests_the_chest_on_the_first_stone_and_fills_the_open_neighbours() {
        let mut region = region();
        region.blocks.set(BlockPos::new(9, 5, 9), WATER);
        region.blocks.set(BlockPos::new(10, 5, 9), WATER);
        region.blocks.set(BlockPos::new(10, 4, 9), AIR);
        let mut entities = Vec::new();
        let mut canvas = PieceCanvas {
            volume: &mut region,
            entities: &mut entities,
            bounds: BoundingBox::point(BlockPos::new(9, 90, 9)),
            orientation: None,
            clip: BoundingBox {
                min: BlockPos::new(0, 1, 0),
                max: BlockPos::new(15, 31, 15),
            },
        };
        let mut rng = LegacyRandom::new(7);
        paint_buried_treasure(&blocks(), &mut canvas, &mut rng);
        assert_eq!(
            region.writes,
            vec![
                (BlockPos::new(10, 5, 9), STONE),
                (BlockPos::new(9, 5, 9), CHEST),
            ]
        );
        assert_eq!(entities.len(), 1);
        assert_eq!(rng, {
            let mut replay = LegacyRandom::new(7);
            replay.next_java_long();
            replay
        });
    }

    #[test]
    fn a_column_with_no_resting_block_places_nothing_and_draws_nothing() {
        let mut region = region();
        for y in 0..=4 {
            region.blocks.fill_layer(y, GRAVEL);
        }
        let mut entities = Vec::new();
        let mut canvas = PieceCanvas {
            volume: &mut region,
            entities: &mut entities,
            bounds: BoundingBox::point(BlockPos::new(9, 90, 9)),
            orientation: None,
            clip: BoundingBox {
                min: BlockPos::new(0, 1, 0),
                max: BlockPos::new(15, 31, 15),
            },
        };
        let mut rng = LegacyRandom::new(7);
        paint_buried_treasure(&blocks(), &mut canvas, &mut rng);
        assert!(region.writes.is_empty());
        assert!(entities.is_empty());
        assert_eq!(rng, LegacyRandom::new(7));
    }
}
