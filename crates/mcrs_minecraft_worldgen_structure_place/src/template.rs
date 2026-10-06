use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_core::{BlockPos, BoundingBox, Mirror};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_registry::StaticKey;
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature::proto::{
    ProcessorRule, RuleBlockEntityModifier, StructureProcessor,
};
use mcrs_minecraft_worldgen_feature::rule_test::RuleTest;
use mcrs_minecraft_worldgen_feature::spawn_condition::SpawnContext;
use mcrs_minecraft_worldgen_feature::template::data_markers;
use mcrs_minecraft_worldgen_feature::tree::UnitFloat;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::{GeneratedEntity, drowned};
use mcrs_minecraft_worldgen_feature_place::template::{ChainKind, CompiledChain, compile_chain};
use mcrs_minecraft_worldgen_structure::OceanTemperature;
use mcrs_minecraft_worldgen_structure::frozen::{FrozenStructures, OceanRuinConfig, StructureId};
use mcrs_minecraft_worldgen_structure::piece::OceanRuinPiece;

use crate::{place_positional, state};

/// The integrities `OceanRuinPieces.addPieces` hands its pieces: the large
/// and small base ruins, then the cracked and mossy overlays.
const INTEGRITIES: [f32; 4] = [0.9, 0.8, 0.7, 0.5];

/// `OceanRuinPieces.makeSettings` for one temperature: the rot at each
/// integrity, structure blocks and air ignored, and five archaeology blocks
/// capped per piece.
#[derive(Clone, Debug)]
pub struct OceanRuinBlocks {
    chains: Vec<(f32, CompiledChain)>,
    chest: VoxelId,
    chest_waterlogged: VoxelId,
}

impl OceanRuinBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        temp: OceanTemperature,
        world_seed: i64,
    ) -> Result<Self, FeatureCompileError> {
        let (candidate, replacement, loot) = match temp {
            OceanTemperature::Warm => (
                keys::block::SAND,
                keys::block::SUSPICIOUS_SAND,
                keys::loot_table::ARCHAEOLOGY_OCEAN_RUIN_WARM,
            ),
            OceanTemperature::Cold => (
                keys::block::GRAVEL,
                keys::block::SUSPICIOUS_GRAVEL,
                keys::loot_table::ARCHAEOLOGY_OCEAN_RUIN_COLD,
            ),
        };
        let bare = |block: StaticKey<keys::Block>| BlockState::bare(block.location().to_arc());
        let chains = INTEGRITIES
            .into_iter()
            .map(|integrity| {
                let list = [
                    StructureProcessor::BlockRot {
                        rottable_blocks: None,
                        integrity: UnitFloat(f64::from(integrity)),
                    },
                    StructureProcessor::BlockIgnore {
                        blocks: vec![bare(keys::block::STRUCTURE_BLOCK), bare(keys::block::AIR)],
                    },
                    StructureProcessor::Capped {
                        delegate: Box::new(StructureProcessor::Rule {
                            rules: vec![ProcessorRule {
                                input_predicate: RuleTest::BlockMatch {
                                    block: candidate.location().to_arc(),
                                },
                                location_predicate: RuleTest::AlwaysTrue,
                                position_predicate: None,
                                output_state: bare(replacement),
                                block_entity_modifier: Some(RuleBlockEntityModifier::AppendLoot {
                                    loot_table: loot.location().to_arc(),
                                }),
                            }],
                        }),
                        limit: IntProvider::Constant(5),
                    },
                ];
                Ok((
                    integrity,
                    compile_chain(&list, ChainKind::Feature, blocks, world_seed)?,
                ))
            })
            .collect::<Result<_, FeatureCompileError>>()?;
        Ok(OceanRuinBlocks {
            chains,
            chest: state(blocks, keys::block::CHEST, &[])?,
            chest_waterlogged: state(blocks, keys::block::CHEST, &[("waterlogged", "true")])?,
        })
    }

    fn chain(&self, integrity: f32) -> &CompiledChain {
        &self
            .chains
            .iter()
            .find(|(at, _)| *at == integrity)
            .expect("the layout hands out one of the four integrities")
            .1
    }
}

/// `OceanRuinPiece.postProcess` for one column: the template at its fixed
/// floor, then the chest and drowned markers the column holds.
#[allow(clippy::too_many_arguments)]
pub fn place_ocean_ruin<W: WorldGenVolume>(
    blocks: &OceanRuinBlocks,
    frozen: &FrozenStructures,
    config: &OceanRuinConfig,
    structure: StructureId,
    piece: &OceanRuinPiece,
    reference: IVec3,
    clip: BoundingBox,
    region: &mut W,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
    rng: &mut WorldgenRandom,
) {
    let position = piece.position.with_y(piece.floor_y);
    let template = &frozen.templates[piece.template.0 as usize];
    let manifest = &frozen.manifests[piece.template.0 as usize];
    let Some(palette) = place_positional(
        template,
        position,
        piece.rotation,
        Mirror::None,
        IVec3::ZERO,
        clip,
        blocks.chain(piece.integrity),
        true,
        true,
        reference,
        region,
        rng,
        entities,
        spawns,
    ) else {
        return;
    };
    let markers = manifest.markers.get(palette).map_or(&[][..], Vec::as_slice);
    for (pos, marker) in data_markers(
        markers,
        position,
        Mirror::None,
        piece.rotation,
        IVec3::ZERO,
        Some(clip),
    ) {
        let pos = BlockPos::from(pos);
        match marker {
            "chest" => {
                let world = region.world();
                let wet = world.water_fluid.contains(region.get(pos).0 as usize);
                region.set(
                    pos,
                    if wet {
                        blocks.chest_waterlogged
                    } else {
                        blocks.chest
                    },
                );
                let loot = if piece.large {
                    keys::loot_table::CHESTS_UNDERWATER_RUIN_BIG
                } else {
                    keys::loot_table::CHESTS_UNDERWATER_RUIN_SMALL
                };
                entities.push(GeneratedBlockEntity::chest(
                    pos,
                    loot.as_str().to_owned(),
                    rng.next_java_long(),
                ));
            }
            "drowned" => {
                let biome = region.biome(pos);
                let ctx = SpawnContext {
                    structure: Some(structure.0),
                    biome,
                    moon_brightness: 1.0,
                };
                let frequent = config.frequent_drowned.contains(usize::from(biome));
                spawns.push(drowned(pos, &ctx, &frozen.variants, frequent, rng));
                let world = region.world();
                let fill = if pos.y > region.extent().sea_level {
                    world.air
                } else {
                    world.water
                };
                region.set(pos, fill);
            }
            _ => {}
        }
    }
}
