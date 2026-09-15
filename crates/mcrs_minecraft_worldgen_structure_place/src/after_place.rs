use bevy_math::IVec3;
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
