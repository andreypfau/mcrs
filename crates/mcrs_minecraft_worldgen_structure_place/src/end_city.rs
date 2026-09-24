use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, BoundingBox, Direction, Mirror};
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature::template::data_markers;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::{GeneratedEntity, elytra_frame, shulker};
use mcrs_minecraft_worldgen_feature_place::template::{CompiledChain, CompiledProcessor};
use mcrs_minecraft_worldgen_structure::frozen::FrozenStructures;
use mcrs_minecraft_worldgen_structure::piece::EndCityPiece;

use crate::template_piece::seed_container_loot;
use crate::{block_mask, place_positional};

pub const END_CITY_TREASURE_LOOT: &str = "minecraft:chests/end_city_treasure";

/// `EndCityPiece.makeSettings`: the two processor chains its pieces place
/// with, by whether the template's air overwrites what stands there.
#[derive(Clone, Debug)]
pub struct EndCityChains {
    overwrite: CompiledChain,
    keep_air: CompiledChain,
}

impl EndCityChains {
    pub fn compile(blocks: &dyn BlockResolver) -> Result<Self, FeatureCompileError> {
        Ok(EndCityChains {
            overwrite: vec![CompiledProcessor::BlockIgnore(block_mask(
                blocks,
                &["minecraft:structure_block"],
            )?)],
            keep_air: vec![CompiledProcessor::BlockIgnore(block_mask(
                blocks,
                &["minecraft:air", "minecraft:structure_block"],
            )?)],
        })
    }
}

/// `EndCityPiece.postProcess` for one column: the template, then its data
/// markers in block order — a chest's loot table under a `Chest` marker, a
/// shulker at a `Sentry`, the elytra frame at `Elytra` facing the piece's south.
#[allow(clippy::too_many_arguments)]
pub fn place_end_city_piece<W: WorldGenVolume>(
    chains: &EndCityChains,
    frozen: &FrozenStructures,
    piece: &EndCityPiece,
    reference: IVec3,
    clip: BoundingBox,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
) {
    let template = &frozen.templates[piece.template.0 as usize];
    let manifest = &frozen.manifests[piece.template.0 as usize];
    let chain = if piece.overwrite {
        &chains.overwrite
    } else {
        &chains.keep_air
    };
    let Some(palette) = place_positional(
        template,
        piece.position,
        piece.rotation,
        Mirror::None,
        IVec3::ZERO,
        clip,
        chain,
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
    for (pos, metadata) in data_markers(
        markers,
        piece.position,
        Mirror::None,
        piece.rotation,
        IVec3::ZERO,
        None,
    ) {
        if metadata.starts_with("Chest") {
            let chest = BlockPos::from(pos - IVec3::Y);
            if clip.is_inside(chest) {
                seed_container_loot(entities, chest, END_CITY_TREASURE_LOOT, rng);
            }
        } else if clip.is_inside(pos.into()) {
            if metadata.starts_with("Sentry") {
                spawns.push(shulker(pos.into(), rng));
            } else if metadata.starts_with("Elytra") {
                let facing = piece.rotation.rotate(Direction::South);
                spawns.push(elytra_frame(pos.into(), facing, rng));
            }
        }
    }
}
