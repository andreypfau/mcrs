use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, BoundingBox, Mirror};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_random::{Random, block_pos_seed};
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature::template::FrozenTemplate;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_feature_place::template::{
    CompiledChain, CompiledProcessor, Placement, SettingsRandom, place_template, rotate_state,
};
use mcrs_minecraft_worldgen_structure::orient::random_rotation;
use mcrs_minecraft_worldgen_structure::piece::NetherFossilPiece;

use crate::{block_mask, state};

#[derive(Clone, Debug)]
pub struct NetherFossilBlocks {
    pub chain: CompiledChain,
    pub dried_ghast: VoxelId,
    pub world_seed: i64,
}

impl NetherFossilBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world_seed: i64,
    ) -> Result<Self, FeatureCompileError> {
        Ok(NetherFossilBlocks {
            chain: vec![CompiledProcessor::BlockIgnore(block_mask(
                blocks,
                &["minecraft:air", "minecraft:structure_block"],
            )?)],
            dried_ghast: state(blocks, "minecraft:dried_ghast", &[])?,
            world_seed,
        })
    }
}

/// `NetherFossilPiece.postProcess` for one column: the clip widened to the
/// whole fossil, so every column the piece touches writes all of it, then
/// the dried ghast drawn positionally at the fossil's centre.
#[allow(clippy::too_many_arguments)]
pub fn paint_nether_fossil<W: WorldGenVolume>(
    b: &NetherFossilBlocks,
    template: &FrozenTemplate,
    piece: &NetherFossilPiece,
    reference: IVec3,
    clip: BoundingBox,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
) {
    if template.palettes.is_empty() {
        return;
    }
    let clip = clip.union(piece.bounds);
    let palette = LegacyRandom::new(block_pos_seed(piece.position))
        .next_i32_bound(template.palettes.len() as i32) as usize;
    place_template(
        &Placement {
            template,
            jigsaws: &[],
            palette,
            position: piece.position,
            reference,
            rotation: piece.rotation,
            mirror: Mirror::None,
            pivot: IVec3::ZERO,
            random: SettingsRandom::Positional,
            clip: Some(clip),
            chain: &b.chain,
            waterlog: true,
            place_entities: true,
        },
        volume,
        rng,
        entities,
        spawns,
    );
    place_dried_ghast(b, volume, piece.bounds, clip);
}

fn place_dried_ghast<W: WorldGenVolume>(
    b: &NetherFossilBlocks,
    volume: &mut W,
    fossil: BoundingBox,
    clip: BoundingBox,
) {
    let centre = *fossil.min + (*fossil.max - *fossil.min + IVec3::ONE) / 2;
    let mut rng = LegacyRandom::new(b.world_seed as u64).fork_at(centre);
    if rng.next_f32() >= 0.5 {
        return;
    }
    let x = fossil.min.x + rng.next_i32_bound(fossil.max.x - fossil.min.x + 1);
    let z = fossil.min.z + rng.next_i32_bound(fossil.max.z - fossil.min.z + 1);
    let pos = BlockPos::new(x, fossil.min.y, z);
    let air = volume
        .world()
        .air_states
        .contains(volume.get(pos).0 as usize);
    if air && clip.is_inside(pos) {
        let rotation = random_rotation(&mut rng);
        let state = rotate_state(volume.world(), b.dried_ghast, rotation);
        volume.set(pos, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_feature::placer::{BoxRegion, mask_of};

    const AIR: VoxelId = VoxelId(0);
    const STONE: VoxelId = VoxelId(1);
    const GHAST: VoxelId = VoxelId(2);

    const FOSSIL: BoundingBox = BoundingBox {
        min: BlockPos::new(10, 12, 10),
        max: BlockPos::new(16, 16, 14),
    };

    fn blocks(world_seed: i64) -> NetherFossilBlocks {
        NetherFossilBlocks {
            chain: Vec::new(),
            dried_ghast: GHAST,
            world_seed,
        }
    }

    fn floored(floor_top: i32) -> BoxRegion {
        let mut region = BoxRegion::new(BlockPos::new(0, 0, 0), BlockPos::new(31, 31, 31), AIR)
            .floor(floor_top, STONE);
        region.world.air_states = mask_of([AIR]);
        region
    }

    fn coin_passes(world_seed: i64) -> bool {
        let centre = *FOSSIL.min + (*FOSSIL.max - *FOSSIL.min + IVec3::ONE) / 2;
        LegacyRandom::new(world_seed as u64)
            .fork_at(centre)
            .next_f32()
            < 0.5
    }

    #[test]
    fn the_dried_ghast_lands_on_the_fossil_floor_when_its_cell_is_air() {
        let passing = (0..).find(|&seed| coin_passes(seed)).unwrap();
        let failing = (0..).find(|&seed| !coin_passes(seed)).unwrap();
        let everything = FOSSIL.inflated(16);

        let mut region = floored(11);
        place_dried_ghast(&blocks(passing), &mut region, FOSSIL, everything);
        let [(pos, state)] = region.writes[..] else {
            panic!("one write, got {:?}", region.writes);
        };
        assert_eq!(state, GHAST);
        assert_eq!(pos.y, FOSSIL.min.y);
        assert!(FOSSIL.is_inside(pos));

        let mut region = floored(11);
        place_dried_ghast(&blocks(failing), &mut region, FOSSIL, everything);
        assert!(region.writes.is_empty());

        let mut region = floored(12);
        place_dried_ghast(&blocks(passing), &mut region, FOSSIL, everything);
        assert!(region.writes.is_empty());

        let mut region = floored(11);
        let elsewhere = BoundingBox::point(BlockPos::new(0, 0, 0));
        place_dried_ghast(&blocks(passing), &mut region, FOSSIL, elsewhere);
        assert!(region.writes.is_empty());
    }
}
