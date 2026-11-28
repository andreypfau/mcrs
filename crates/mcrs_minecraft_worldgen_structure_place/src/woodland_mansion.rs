use glam::IVec3;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BoundingBox, Direction};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature::template::{FrozenTemplate, TemplateManifest, data_markers};
use mcrs_minecraft_worldgen_feature_place::entity::{GeneratedEntity, allays, evoker, vindicator};
use mcrs_minecraft_worldgen_feature_place::template::CompiledChain;
use mcrs_minecraft_worldgen_structure::piece::WoodlandMansionPiece;
use mcrs_voxel::BlockPos;

use crate::canvas::ChestStates;
use crate::{ignore_blocks, place_positional, state};
use mcrs_minecraft_block::block_keys::Block;

#[derive(Clone)]
pub struct WoodlandMansionBlocks {
    pub chain: CompiledChain,
    pub chest: ChestStates,
    pub cobblestone: VoxelId,
}

impl WoodlandMansionBlocks {
    pub fn compile(blocks: &impl BlockResolver) -> Result<Self, FeatureCompileError> {
        Ok(WoodlandMansionBlocks {
            chain: ignore_blocks(blocks, &[Block::StructureBlock])?,
            chest: ChestStates::compile(blocks)?,
            cobblestone: state(blocks, Block::Cobblestone, &[])?,
        })
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::too_many_arguments,
        reason = "independent inputs of one column's placement"
    )
)]
pub fn place_woodland_mansion_piece<W: WorldGenVolume>(
    b: &WoodlandMansionBlocks,
    template: &FrozenTemplate,
    manifest: &TemplateManifest,
    piece: &WoodlandMansionPiece,
    reference: IVec3,
    clip: BoundingBox,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    spawns: &mut Vec<GeneratedEntity>,
) {
    let Some(palette) = place_positional(
        template,
        piece.position,
        piece.rotation,
        piece.mirror,
        IVec3::ZERO,
        clip,
        &b.chain,
        true,
        false,
        reference,
        volume,
        rng,
        spawns,
    ) else {
        return;
    };
    let markers = manifest.markers.get(palette).map_or(&[][..], Vec::as_slice);
    let air = volume.world().air;
    for (pos, marker) in data_markers(
        markers,
        piece.position,
        piece.mirror,
        piece.rotation,
        IVec3::ZERO,
        Some(clip),
    ) {
        let pos = BlockPos::from(pos);
        if marker.starts_with("Chest") {
            let facing = match marker {
                "ChestWest" => piece.rotation.rotate(Direction::West),
                "ChestEast" => piece.rotation.rotate(Direction::East),
                "ChestSouth" => piece.rotation.rotate(Direction::South),
                "ChestNorth" => piece.rotation.rotate(Direction::North),
                _ => Direction::North,
            };
            if volume.holds(&b.chest.states, pos) {
                continue;
            }
            let Some(chest) = b.chest.facing(facing) else {
                continue;
            };
            volume.set(pos, chest);
            volume.add_block_entity(GeneratedBlockEntity::chest(
                pos,
                mcrs_minecraft_loot::loot_table_keys::CHESTS_WOODLAND_MANSION.to_string(),
                rng.next_java_long(),
            ));
            continue;
        }
        let mobs = match marker {
            "Mage" => vec![evoker(pos, rng)],
            "Warrior" => vec![vindicator(pos, rng)],
            "Group of Allays" => allays(pos, rng),
            _ => continue,
        };
        for mob in mobs {
            spawns.push(mob);
            volume.set(pos, air);
        }
    }
}
