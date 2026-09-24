use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, BoundingBox, Direction};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature::template::{FrozenTemplate, TemplateManifest, data_markers};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::{GeneratedEntity, allays, evoker, vindicator};
use mcrs_minecraft_worldgen_feature_place::template::{CompiledChain, CompiledProcessor};
use mcrs_minecraft_worldgen_structure::piece::WoodlandMansionPiece;

use crate::canvas::ChestStates;
use crate::{block_mask, place_positional, state};

pub const WOODLAND_MANSION_LOOT: &str = "minecraft:chests/woodland_mansion";

#[derive(Clone)]
pub struct WoodlandMansionBlocks {
    /// `BlockIgnoreProcessor.STRUCTURE_BLOCK`, the only processor a mansion
    /// piece carries.
    pub chain: CompiledChain,
    pub chest: ChestStates,
    pub cobblestone: VoxelId,
}

impl WoodlandMansionBlocks {
    pub fn compile(blocks: &dyn BlockResolver) -> Result<Self, FeatureCompileError> {
        Ok(WoodlandMansionBlocks {
            chain: vec![CompiledProcessor::BlockIgnore(block_mask(
                blocks,
                &["minecraft:structure_block"],
            )?)],
            chest: ChestStates::compile(blocks)?,
            cobblestone: state(blocks, "minecraft:cobblestone", &[])?,
        })
    }
}

/// `WoodlandMansionPiece.postProcess` for one column: the template, then its
/// data markers inside the clip in palette order, a chest facing as the
/// marker names it, or the mobs the marker spawns with the block under them
/// cleared.
#[allow(clippy::too_many_arguments)]
pub fn place_woodland_mansion_piece<W: WorldGenVolume>(
    b: &WoodlandMansionBlocks,
    template: &FrozenTemplate,
    manifest: &TemplateManifest,
    piece: &WoodlandMansionPiece,
    reference: IVec3,
    clip: BoundingBox,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
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
        entities,
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
            let index = Direction::HORIZONTAL
                .iter()
                .position(|d| *d == facing)
                .expect("a horizontal facing");
            volume.set(pos, b.chest.facing[index]);
            entities.push(GeneratedBlockEntity::chest(
                pos,
                WOODLAND_MANSION_LOOT.to_owned(),
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
