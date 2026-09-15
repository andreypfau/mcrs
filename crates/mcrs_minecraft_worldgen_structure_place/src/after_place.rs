use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, shuffle};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_structure::orient::Orientation;

use crate::scattered::{
    DESERT_PYRAMID_ARCHAEOLOGY_LOOT, DesertPyramidBlocks, collapsed_roof_pos,
    potential_suspicious_sand,
};
use crate::woodland_mansion::WoodlandMansionBlocks;

/// `DesertPyramidStructure.afterPlace` for one column: the marked roof cell,
/// then the cellar's sand, five to eight cells of it suspicious, in a shuffle
/// drawn positionally at the piece's centre.
pub fn desert_pyramid<W: WorldGenVolume>(
    b: &DesertPyramidBlocks,
    volume: &mut W,
    entities: &mut Vec<GeneratedBlockEntity>,
    clip: BoundingBox,
    bounds: BoundingBox,
    orientation: Orientation,
) {
    let suspicious = |volume: &mut W, entities: &mut Vec<GeneratedBlockEntity>, pos: BlockPos| {
        if !clip.is_inside(pos) {
            return;
        }
        volume.set(pos, b.suspicious_sand);
        entities.push(GeneratedBlockEntity::BrushableBlock {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            loot_table: Some(DESERT_PYRAMID_ARCHAEOLOGY_LOOT.to_owned()),
            loot_table_seed: pos.as_long(),
            item: None,
            components: None,
        });
    };

    suspicious(
        volume,
        entities,
        collapsed_roof_pos(b.world_seed, bounds, orientation),
    );

    let mut placements: Vec<BlockPos> = potential_suspicious_sand(bounds, orientation).collect();
    placements.sort_by_key(|pos| (pos.y, pos.z, pos.x));
    placements.dedup();
    let centre = *bounds.min + (*bounds.max - *bounds.min + IVec3::ONE) / 2;
    let mut rng = LegacyRandom::new(b.world_seed as u64).fork_at(centre);
    shuffle(&mut placements, &mut rng);
    let mut to_place = placements.len().min(rng.next_i32_bound(3) as usize + 5);
    let sand = b.sand.get(orientation);
    for pos in placements {
        if to_place > 0 {
            to_place -= 1;
            suspicious(volume, entities, pos);
        } else if clip.is_inside(pos) {
            volume.set(pos, sand);
        }
    }
}

/// `WoodlandMansionStructure.afterPlace` for one column: under every cell of
/// the column that is inside a piece and not empty at the start's floor,
/// cobblestone fills down through air and liquid to the first solid block.
pub fn woodland_mansion<W: WorldGenVolume>(
    b: &WoodlandMansionBlocks,
    volume: &mut W,
    clip: BoundingBox,
    piece_bounds: &[BoundingBox],
) {
    let bounds = piece_bounds
        .iter()
        .copied()
        .reduce(BoundingBox::union)
        .expect("a start has at least one piece");
    let min_y = volume.extent().min_y;
    let floor = bounds.min.y;
    let world = volume.world();
    let (air, water, lava) = (
        world.air_states.clone(),
        world.water_states.clone(),
        world.lava_states.clone(),
    );
    let empty = |state: VoxelId| air.contains(state.0 as usize);
    let liquid =
        |state: VoxelId| water.contains(state.0 as usize) || lava.contains(state.0 as usize);
    for x in clip.min.x..=clip.max.x {
        for z in clip.min.z..=clip.max.z {
            let pos = BlockPos::new(x, floor, z);
            if empty(volume.get(pos))
                || !bounds.is_inside(pos)
                || !piece_bounds.iter().any(|piece| piece.is_inside(pos))
            {
                continue;
            }
            for y in (min_y + 1..floor).rev() {
                let pos = BlockPos::new(x, y, z);
                let state = volume.get(pos);
                if !empty(state) && !liquid(state) {
                    break;
                }
                volume.set(pos, b.cobblestone);
            }
        }
    }
}
