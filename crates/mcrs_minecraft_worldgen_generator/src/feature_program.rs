use crate::beta_ores::{BetaOreBlockIds, apply_beta_ores_in};
use crate::features::FeatureTables;
use crate::structures::{check_block_entity_ids, resolve_palette_state};
use crate::trees::{
    build_tree_tables, compile_decorator, compile_provider, compile_tree, state_of, with_property,
};
use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{
    BlockDefinitions, BlockEntry, BlockStateData, BlockStateFlags, FluidId,
};
use mcrs_minecraft_block::{Block as VanillaBlock, Fluid};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_core::value_provider::{IntProvider as IntProviderRef, pick_weighted_by};
use mcrs_minecraft_core::voxel_shape::{FACE_MASK_FULL, VoxelShape};
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_core::{Mirror, Rotation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_feature::block_predicate::Direction;
use mcrs_minecraft_worldgen_feature::compile::{
    BlockResolver, FeatureCompileError, LoadedFeatures, StateQuery, compile_placement,
    compile_predicate, compile_rule, state_named, state_of as resolve_state, states_of,
};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{
    BiomeMask, BlockLayout, Modifier, PlacerScratch, Predicate, PropertyLayout, StateMask,
    WorldGenVolume, WorldStates, place,
};
use mcrs_minecraft_worldgen_feature::proto::{
    BlockReplacement, Feature, Holder, PlacedFeature, PlacedFeatureSet, StructureProcessor,
    StructureProcessorList, WeightedPlacedFeature, processor_list,
};
use mcrs_minecraft_worldgen_feature::template::{
    FrozenTemplate, TemplateManifest, bounding_box, zero_position_with_transform,
};
use mcrs_minecraft_worldgen_feature_place::bamboo::{CompiledBamboo, place_bamboo};
use mcrs_minecraft_worldgen_feature_place::blob::{
    CompiledBlockBlob, CompiledDelta, CompiledReplaceBlobs, place_block_blob, place_delta,
    place_replace_blobs,
};
use mcrs_minecraft_worldgen_feature_place::block_column::{
    ColumnLayer, CompiledBlockColumn, place_block_column,
};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::block_pile::{CompiledBlockPile, place_block_pile};
use mcrs_minecraft_worldgen_feature_place::chorus_plant::{
    CompiledChorusPlant, place_chorus_plant,
};
use mcrs_minecraft_worldgen_feature_place::coral::{place_coral_claw, place_coral_tree};
use mcrs_minecraft_worldgen_feature_place::end::{
    CompiledEndGateway, CompiledEndIsland, CompiledEndPlatform, CompiledEndPodium,
    CompiledEndSpikes, CompiledVoidStartPlatform, place_end_gateway, place_end_island,
    place_end_platform, place_end_podium, place_end_spike, place_void_start_platform, seed_spikes,
};
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_feature_place::fallen_tree::{CompiledFallenTree, place_fallen_tree};
use mcrs_minecraft_worldgen_feature_place::fill_layer::{CompiledFillLayer, place_fill_layer};
use mcrs_minecraft_worldgen_feature_place::geode::{CompiledGeode, GeodeCrystal, place_geode};
use mcrs_minecraft_worldgen_feature_place::huge_fungus::{CompiledHugeFungus, place_huge_fungus};
use mcrs_minecraft_worldgen_feature_place::huge_mushroom::{
    CompiledHugeMushroom, MushroomCap, MushroomFaces, place_huge_mushroom,
};
use mcrs_minecraft_worldgen_feature_place::iceberg::{CompiledIceberg, place_iceberg};
use mcrs_minecraft_worldgen_feature_place::lake::{CompiledLake, place_lake};
use mcrs_minecraft_worldgen_feature_place::mossy_carpet::{
    CarpetShape, MossyCarpetStates, SHAPE_COUNT, WallSide,
};
use mcrs_minecraft_worldgen_feature_place::multiface_growth::{
    CompiledMultifaceGrowth, MultifaceStates, place_multiface_growth, valid_directions,
};
use mcrs_minecraft_worldgen_feature_place::neighbor_spread::{
    CompiledNeighborSpread, place_random_neighbor_spread,
};
use mcrs_minecraft_worldgen_feature_place::ore_modern::{
    CompiledOre, OreReplacement, OreScratch, place_modern_ore,
};
use mcrs_minecraft_worldgen_feature_place::patch::{
    CompiledVegetationPatch, Waterlogging, place_vegetation_patch,
};
use mcrs_minecraft_worldgen_feature_place::projected_patchy_square::{
    CompiledProjectedPatchySquare, place_projected_random_patchy_square,
};
use mcrs_minecraft_worldgen_feature_place::replace_single_block::{
    CompiledReplaceSingleBlock, Replacement, place_replace_single_block,
};
use mcrs_minecraft_worldgen_feature_place::room::{
    CompiledBonusChest, CompiledMonsterRoom, place_bonus_chest, place_monster_room,
};
use mcrs_minecraft_worldgen_feature_place::root_system::{CompiledRootSystem, place_root_system};
use mcrs_minecraft_worldgen_feature_place::scattered_ore::place_scattered_ore;
use mcrs_minecraft_worldgen_feature_place::sculk_patch::{CompiledSculkPatch, place_sculk_patch};
use mcrs_minecraft_worldgen_feature_place::simple_block::{
    CompiledSimpleBlock, DoublePlant, place_simple_block,
};
use mcrs_minecraft_worldgen_feature_place::single_block_pillar::{
    CompiledSingleBlockPillar, place_single_block_pillar,
};
use mcrs_minecraft_worldgen_feature_place::speleothem::{
    CompiledLargeDripstone, CompiledSpeleothemCluster, PointedStates, place_large_dripstone,
    place_speleothem_cluster,
};
use mcrs_minecraft_worldgen_feature_place::speleothem_single::{
    CompiledSpeleothem, place_speleothem,
};
use mcrs_minecraft_worldgen_feature_place::spike::{CompiledSpike, place_spike};
use mcrs_minecraft_worldgen_feature_place::spring::{CompiledSpring, place_spring};
use mcrs_minecraft_worldgen_feature_place::stepped_column::{
    CompiledSteppedColumnCluster, place_stepped_column_cluster,
};
use mcrs_minecraft_worldgen_feature_place::tables::BlockTables;
use mcrs_minecraft_worldgen_feature_place::template::{
    ChainKind, CompiledChain, Placement, SettingsRandom, compile_chain, place_template,
};
use mcrs_minecraft_worldgen_feature_place::terrain_skin::{
    BiomeClimate, CompiledBlueIce, CompiledDisk, CompiledFreezeTopLayer, CompiledUnderwaterMagma,
    place_blue_ice, place_disk, place_freeze_top_layer, place_underwater_magma,
};
use mcrs_minecraft_worldgen_feature_place::tree::decorator::TreeSink;
use mcrs_minecraft_worldgen_feature_place::tree::provider::StateProvider;
use mcrs_minecraft_worldgen_feature_place::tree::{CompiledTree, TreeTables, place_tree};
use mcrs_minecraft_worldgen_feature_place::vines::place_vines;
use mcrs_minecraft_worldgen_structure::frozen::{
    ElementId, FrozenElement, FrozenStructure, FrozenStructures, StructureId, StructureKind,
};
use mcrs_minecraft_worldgen_structure::{DecorationStep, LiquidSettings};
use mcrs_minecraft_worldgen_structure_place::buried_treasure::BuriedTreasureBlocks;
use mcrs_minecraft_worldgen_structure_place::fortress::FortressBlocks;
use mcrs_minecraft_worldgen_structure_place::jungle_temple::JungleTempleBlocks;
use mcrs_minecraft_worldgen_structure_place::portal::RuinedPortalBlocks;
use mcrs_minecraft_worldgen_structure_place::ocean_monument::OceanMonumentBlocks;
use mcrs_minecraft_worldgen_structure_place::mineshaft::MineshaftBlocks;
use mcrs_minecraft_worldgen_structure_place::nether_fossil::NetherFossilBlocks;
use mcrs_minecraft_worldgen_structure_place::scattered::DesertPyramidBlocks;
use mcrs_minecraft_worldgen_structure_place::template::OceanRuinBlocks;
use mcrs_minecraft_worldgen_structure_place::template_piece::{
    IglooBlocks, ignore_structure_and_air,
};
use rustc_hash::FxHashMap;
use std::ops::Range;
use std::sync::Arc;

/// What one feature of a step actually runs, once every name in it is a number.
pub enum Generator {
    Ore(CompiledOre),
    Tree(Box<CompiledTree>),
    /// `random_selector`: one draw per entry in list order, the first hit
    /// placing that nested placed feature through its own modifier chain.
    RandomSelector {
        features: Vec<(f32, Nested)>,
        default: Box<Nested>,
    },
    /// `weighted_random_selector`: one draw over the total weight, then the
    /// cumulative weights walked in list order. An empty list places nothing
    /// and draws nothing.
    ///
    /// `simple_random_selector` compiles to this with every weight 1: the
    /// reference's `nextInt(size)` and the cumulative walk over `size` ones
    /// pick the same entry from the same single draw.
    WeightedRandomSelector(Vec<(i32, Nested)>),
    /// `random_boolean_selector`: one `nextBoolean`, which on a Xoroshiro
    /// source is the low bit of a whole long rather than a bounded int.
    RandomBooleanSelector {
        feature_true: Box<Nested>,
        feature_false: Box<Nested>,
    },
    FallenTree(Box<CompiledFallenTree>),
    /// The nested tree runs its own placed feature, so the generator carries it
    /// beside the configuration rather than resolving it.
    RootSystem {
        config: Box<CompiledRootSystem>,
        feature: Box<Nested>,
    },
    SimpleBlock(Box<CompiledSimpleBlock>),
    BlockColumn(Box<CompiledBlockColumn>),
    BlockPile(CompiledBlockPile),
    MultifaceGrowth(Box<CompiledMultifaceGrowth>),
    Vines([VoxelId; 6]),
    RandomNeighborSpread(Box<CompiledNeighborSpread>),
    /// Every entry runs, whatever the ones before it did.
    Overlay(Vec<Nested>),
    /// The first entry that places nothing ends the run, so an entry this build
    /// cannot place would swallow every later entry's draws; the compile refuses
    /// a sequence unless all of it runs.
    Sequence(Vec<Nested>),
    ScatteredOre(CompiledOre),
    VegetationPatch {
        config: Box<CompiledVegetationPatch>,
        feature: Box<Nested>,
    },
    Disk(Box<CompiledDisk>),
    BlockBlob(CompiledBlockBlob),
    ReplaceBlobs(CompiledReplaceBlobs),
    Delta(Box<CompiledDelta>),
    UnderwaterMagma(CompiledUnderwaterMagma),
    BlueIce(CompiledBlueIce),
    FreezeTopLayer(Box<CompiledFreezeTopLayer>),
    Spring(CompiledSpring),
    MonsterRoom(Box<CompiledMonsterRoom>),
    BonusChest(CompiledBonusChest),
    Lake(Box<CompiledLake>),
    Geode(Box<CompiledGeode>),
    SpeleothemCluster(Box<CompiledSpeleothemCluster>),
    LargeDripstone(Box<CompiledLargeDripstone>),
    Bamboo(CompiledBamboo),
    ChorusPlant(Box<CompiledChorusPlant>),
    HugeFungus(Box<CompiledHugeFungus>),
    HugeMushroom(Box<CompiledHugeMushroom>),
    Iceberg(CompiledIceberg),
    Spike(CompiledSpike),
    Speleothem(Box<CompiledSpeleothem>),
    /// The nested feature places each block of the shape, so a refusal from it
    /// is what ends a trunk or a branch.
    CoralTree(Box<Nested>),
    CoralClaw(Box<Nested>),
    /// A pillar with no `cap_feature` still runs to the end of its column; the
    /// cap is what the corpus hangs the surrounding scatter off.
    SingleBlockPillar {
        config: Box<CompiledSingleBlockPillar>,
        cap: Option<Box<Nested>>,
    },
    ProjectedPatchySquare(Box<CompiledProjectedPatchySquare>),
    SculkPatch(Box<CompiledSculkPatch>),
    SteppedColumnCluster(Box<CompiledSteppedColumnCluster>),
    /// Places nothing and draws nothing, which is not the same as having no
    /// generator: a sequence ends at the first entry that places nothing, and
    /// this one succeeds.
    NoOp,
    EndPlatform(CompiledEndPlatform),
    VoidStartPlatform(CompiledVoidStartPlatform),
    EndPodium(CompiledEndPodium),
    EndGateway(CompiledEndGateway),
    EndIsland(CompiledEndIsland),
    EndSpikes(Box<CompiledEndSpikes>),
    FillLayer(CompiledFillLayer),
    /// Always reports success, so a sequence holding one does not end on it.
    ReplaceSingleBlock(CompiledReplaceSingleBlock),
    BetaPopulate(Box<BetaPopulate>),
    Template(Box<CompiledTemplateFeature>),
    Fossil(Box<CompiledFossil>),
}

/// `minecraft:template`: one weighted draw picks the template, one bounded
/// draw its rotation, and the palette and loot seeds come off the same stream.
// ponytail: the reference leaves `knownShape` false here and re-derives every
// placed block's shape from its neighbours afterwards, which a sulfur spike at
// a template's edge can feel; the upgrade is that post pass over the region.
pub struct CompiledTemplateFeature {
    entries: Vec<(i32, FrozenTemplate, Vec<Rotation>)>,
    chain: CompiledChain,
}

/// `minecraft:fossil`: one draw picks the rotation, one the fossil and its
/// overlay, one the depth below the lowest ocean floor under the footprint;
/// then both templates place at that corner with their rot processors
/// drawing from the same stream.
pub struct CompiledFossil {
    pairs: Vec<(FrozenTemplate, FrozenTemplate)>,
    fossil_chain: CompiledChain,
    overlay_chain: CompiledChain,
    max_empty_corners: usize,
}

/// One template pool element with every name in it resolved: what a jigsaw
/// piece places.
pub enum CompiledElement {
    Single {
        template: Arc<FrozenTemplate>,
        manifest: Arc<TemplateManifest>,
        /// `None` when the processor list has a shape this build cannot run;
        /// the piece then places nothing.
        chain: Option<Arc<CompiledChain>>,
        liquid: Option<LiquidSettings>,
    },
    List(Vec<ElementId>),
    Feature(Nested),
    Empty,
}

/// One hardcoded structure type's tables with every name resolved: what its
/// pieces place. Each type adds its variant with its generator.
#[derive(Clone)]
pub enum CompiledStructure {
    DesertPyramid(Box<DesertPyramidBlocks>),
    JungleTemple(Box<JungleTempleBlocks>),
    BuriedTreasure(Box<BuriedTreasureBlocks>),
    Fortress(Box<FortressBlocks>),
    Shipwreck(CompiledChain),
    OceanRuin(Box<OceanRuinBlocks>),
    RuinedPortal(Box<RuinedPortalBlocks>),
    OceanMonument(Box<OceanMonumentBlocks>),
    Mineshaft(Box<MineshaftBlocks>),
    Igloo(Box<IglooBlocks>),
    NetherFossil(Box<NetherFossilBlocks>),
}

/// Beta's populate step for the column the origin is in. It draws from one
/// legacy stream seeded by the chunk, never from the source its step hands it.
pub struct BetaPopulate {
    ids: BetaOreBlockIds,
    world_seed: i64,
}

/// A placed feature written inside another feature: its own chain, its own
/// generator, and no place in any step's index.
///
/// A nested feature this build cannot run keeps its entry, its chance draw and
/// its chain's draws, and places nothing. Unlike a skipped top-level feature,
/// which owns a seeded source of its own, it draws from the parent's source, so
/// spending fewer draws there moves the parent's every later object.
pub struct Nested {
    pub placement: Vec<Modifier>,
    pub generator: Option<Generator>,
}

/// One dimension's decoration program, ready to run.
pub struct FeatureProgram {
    /// Index-aligned with [`FeatureTables::steps`], so a feature's position is
    /// still the index its seed is drawn from. A feature this build has no
    /// code for keeps its slot with no generator and is skipped, so the seeds
    /// of everything after it are unchanged.
    features: Vec<Vec<Nested>>,
    per_biome: Vec<Vec<FixedBitSet>>,
    /// Per step, the token of each entry's placed feature.
    token: Vec<Vec<usize>>,
    /// Per biome slot, a bit per token it names at any step. The reference's
    /// `hasFeature` is a set over the biome's whole list, so the `biome` filter
    /// is step-agnostic even though the candidate set of a step is not.
    carried: Vec<FixedBitSet>,
    /// The palette's biome byte to its row of `per_biome`. A biome the source
    /// cannot answer with has no row, so a lookup is also the intersection with
    /// `possibleBiomes`.
    biome_slot: [Option<u16>; 256],
    trees: Arc<TreeTables>,
    /// `pale_moss` runs `minecraft:pale_moss_patch` as a bare feature on the
    /// tree's own source, so it is one program-wide generator rather than part
    /// of any tree's configuration.
    moss_patch: Option<Box<Generator>>,
    /// Indexed by `ElementId`; empty for a dimension without structures.
    elements: Vec<CompiledElement>,
    /// Indexed by `StructureId`; `None` for a jigsaw structure, whose pieces
    /// are elements, and for a type this build has no generator for.
    structures: Vec<Option<CompiledStructure>>,
    rungs: Arc<[Range<usize>]>,
    pub world: Arc<WorldStates>,
    /// Indexed by the biome id [`WorldGenVolume::biome`] answers with.
    pub climate: Arc<[BiomeClimate]>,
}

/// `GenerationStep.Decoration.values().length`: the reference walks at least
/// this many steps whether or not any biome lists features for them, and a
/// structure's step may lie past the last feature step.
const DECORATION_STEPS: usize = DecorationStep::TopLayerModification as usize + 1;

/// The first step of each rung after the first: a column runs the steps of one
/// rung against a neighbourhood in which every earlier rung is already merged,
/// so a cut here is a point where every column in the world agrees on the
/// blocks before any of them reads them again.
///
/// Vegetation is cut off from what shapes the ground because it is the only
/// thing that asks whether a block can stand where it was put: a lake that
/// arrives after a plant leaves the plant in the air. The top layer is cut off
/// from vegetation for the same reason in the other direction — snow settles on
/// what grew.
///
/// Every cut costs two rings of halo around each column, so the list is short
/// on purpose.
const RUNG_STARTS: [usize; 2] = [
    9,  // vegetal_decoration
    10, // top_layer_modification
];

/// The rungs `RUNG_STARTS` cuts the steps into, with the ones this dimension
/// has no feature for dropped: an empty rung is a barrier nothing waits on and
/// two rings of halo nobody needs.
fn rungs_of(steps: &[Vec<Nested>], structures: &[FrozenStructure]) -> Arc<[Range<usize>]> {
    let bounds = std::iter::once(0)
        .chain(RUNG_STARTS)
        .chain(std::iter::once(steps.len()))
        .filter(|bound| *bound <= steps.len());
    let mut cuts: Vec<usize> = bounds.collect();
    cuts.dedup();
    cuts.windows(2)
        .map(|pair| pair[0]..pair[1])
        .filter(|rung| {
            steps[rung.clone()].iter().any(|step| !step.is_empty())
                || structures.iter().any(|s| rung.contains(&(s.step as usize)))
        })
        .collect()
}

impl FeatureProgram {
    /// Resolve every name the tables carry, or say which asset does not
    /// resolve. A feature whose shape this build has no code for is not an
    /// error: it is logged, keeps its slot and places nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        tables: &FeatureTables,
        corpus: &LoadedFeatures,
        blocks: &BlockDefinitions,
        tags: Option<&DynTagRegistry<VanillaBlock>>,
        fluid_tags: Option<&DynTagRegistry<Fluid>>,
        biomes: &RegistrySnapshot<Biome>,
        world_seed: i64,
        structures: Option<&FrozenStructures>,
    ) -> Result<Self, FeatureCompileError> {
        let climate: Vec<BiomeClimate> = (0..biomes.len())
            .map(|id| {
                let entry = biomes.by_id(id).expect("a registry id below its length");
                tables
                    .climate
                    .get(entry.location.as_str())
                    .copied()
                    .ok_or_else(|| FeatureCompileError::UnknownBiomeSet(entry.location.to_string()))
            })
            .collect::<Result<_, _>>()?;
        let resolver = Resolver::new(blocks, tags, fluid_tags, biomes, world_seed, &climate)?;
        let trees = Arc::new(build_tree_tables(&resolver).map_err(|e| e.within("tree tables"))?);
        let mut steps = Vec::with_capacity(tables.features.steps.len());

        for (step, features) in tables.features.steps.iter().enumerate() {
            let mut compiled = Vec::with_capacity(features.len());
            for (index, feature) in features.iter().enumerate() {
                let name = feature
                    .id
                    .as_ref()
                    .map_or_else(|| format!("step {step} index {index}"), |id| id.to_string());
                match compile_placed(&feature.placed, &trees, &resolver, corpus) {
                    Ok(nested) => compiled.push(nested),
                    Err(error) if error.is_unsupported() => {
                        tracing::warn!(feature = %name, %error, "the feature places nothing");
                        compiled.push(Nested {
                            placement: Vec::new(),
                            generator: None,
                        });
                    }
                    Err(error) => return Err(error.within(name)),
                }
            }
            steps.push(compiled);
        }
        steps.resize_with(steps.len().max(DECORATION_STEPS), Vec::new);

        let mut biome_slot = [None; 256];
        for (slot, id) in tables.biome_order.iter().enumerate() {
            if let Some(network) = biomes.by_location(id.as_str())
                && let Ok(byte) = u8::try_from(network)
            {
                biome_slot[usize::from(byte)] = Some(slot as u16);
            }
        }

        let tokens = tables.features.token.iter().flatten().count();
        let carried: Vec<FixedBitSet> = tables
            .features
            .per_biome
            .iter()
            .map(|per_step| {
                let mut bits = FixedBitSet::with_capacity(tokens);
                for (step, present) in per_step.iter().enumerate() {
                    bits.extend(
                        present
                            .ones()
                            .map(|index| tables.features.token[step][index]),
                    );
                }
                bits
            })
            .collect();

        let moss_id = ResourceLocation::parse("minecraft:pale_moss_patch").expect("a literal id");
        let moss_patch = corpus
            .features
            .get(&moss_id)
            .map(|feature| {
                compile_generator(feature, &trees, &resolver, corpus)
                    .map(Box::new)
                    .map_err(|e| e.within(&moss_id))
            })
            .transpose()?;

        let elements = match structures {
            Some(frozen) => compile_elements(frozen, &trees, &resolver, corpus)?,
            None => Vec::new(),
        };
        let compiled_structures = match structures {
            Some(frozen) => compile_structures(frozen, &resolver)?,
            None => Vec::new(),
        };

        Ok(FeatureProgram {
            rungs: rungs_of(
                &steps,
                structures.map_or(&[][..], |f| f.structures.as_slice()),
            ),
            features: steps,
            per_biome: tables.features.per_biome.clone(),
            token: tables.features.token.clone(),
            carried,
            biome_slot,
            moss_patch,
            elements,
            structures: compiled_structures,
            trees,
            world: Arc::new(resolver.world),
            climate: Arc::from(climate),
        })
    }

    pub fn element(&self, id: ElementId) -> &CompiledElement {
        &self.elements[id.0 as usize]
    }

    pub fn structure(&self, id: StructureId) -> Option<&CompiledStructure> {
        self.structures[id.0 as usize].as_ref()
    }

    pub fn chain(&self, step: usize, index: usize) -> &[Modifier] {
        &self.features[step][index].placement
    }

    /// The steps of each rung, in order. A column climbs one rung at a time and
    /// reads its neighbours only between them.
    pub fn rungs(&self) -> &[Range<usize>] {
        &self.rungs
    }

    pub fn slot_of(&self, palette_biome: u32) -> Option<usize> {
        let byte = u8::try_from(palette_biome).ok()?;
        self.biome_slot[usize::from(byte)].map(usize::from)
    }

    /// Whether one biome carries the feature at `(step, index)` — the test the
    /// `biome` placement filter makes.
    ///
    /// The biome names it at any step, not at this one: a placed feature a biome
    /// lists at step 5 passes the filter for the same object reached at step 3.
    pub fn carries(&self, palette_biome: u32, step: usize, index: usize) -> bool {
        self.slot_of(palette_biome).is_some_and(|slot| {
            self.token
                .get(step)
                .and_then(|row| row.get(index))
                .is_some_and(|token| self.carried[slot].contains(*token))
        })
    }

    /// The features of each step the region's biomes ask for.
    pub fn present(&self, slots: &[usize]) -> Vec<FixedBitSet> {
        self.features
            .iter()
            .enumerate()
            .map(|(step, features)| {
                let mut wanted = FixedBitSet::with_capacity(features.len());
                for &slot in slots {
                    if let Some(bits) = self.per_biome[slot].get(step) {
                        wanted.union_with(bits);
                    }
                }
                wanted
            })
            .collect()
    }

    /// `scratch` is the pool of placer stacks nested features borrow from,
    /// handed back by [`Run::finish`] so a worker keeps them warm.
    pub fn run(&self, scratch: RunScratch) -> Run<'_> {
        Run {
            entities: Vec::new(),
            spawns: Vec::new(),
            moss_patch: self.moss_patch.as_deref(),
            scratch,
        }
    }

    pub fn generator_at(&self, step: usize, index: usize) -> Option<&Generator> {
        self.features.get(step)?.get(index)?.generator.as_ref()
    }

    /// `BlockState.canSurvive` for the state at a position. A block outside the
    /// families overrides nothing, and `BlockBehaviour.canSurvive` is true.
    pub fn would_survive(
        &self,
        block_index: u32,
        p: BlockPos,
        get: impl Fn(BlockPos) -> VoxelId,
    ) -> bool {
        self.trees
            .survive
            .get(&block_index)
            .is_none_or(|rule| rule.test(p, get))
    }
}

/// What every generator of one column's run shares: the block entities it
/// grows, the entities it spawns and the bare `pale_moss_patch` a tree
/// decorator runs on the tree's own source.
pub struct Run<'a> {
    pub entities: Vec<GeneratedBlockEntity>,
    pub spawns: Vec<GeneratedEntity>,
    pub moss_patch: Option<&'a Generator>,
    scratch: RunScratch,
}

/// The buffers one column's run refills rather than reallocates: the stack a
/// nested placement takes one of, and the ore vein's own pair.
#[derive(Default)]
pub struct RunScratch {
    placers: Vec<PlacerScratch>,
    ore: OreScratch,
}

impl Run<'_> {
    pub fn finish(self) -> (Vec<GeneratedBlockEntity>, Vec<GeneratedEntity>, RunScratch) {
        (self.entities, self.spawns, self.scratch)
    }
}

/// What a tree writes beside its blocks: its block entities join the run's, and
/// its moss patch is the program's own `pale_moss_patch` on the tree's source.
struct TreeRun<'r, 'a, 'c> {
    run: &'r mut Run<'a>,
    carries: &'c dyn Fn(u32) -> bool,
}

impl<W: WorldGenVolume> TreeSink<W> for TreeRun<'_, '_, '_> {
    fn block_entity(&mut self, entity: GeneratedBlockEntity) {
        self.run.entities.push(entity);
    }

    fn moss_patch(&mut self, region: &mut W, rng: &mut XoroshiroRandom, at: BlockPos) {
        // A tree that decorates with moss made the build resolve the patch, so
        // nothing to run here is a bug in the compile.
        if let Some(generator) = self.run.moss_patch {
            generator.place(self.run, region, rng, at, self.carries);
        }
    }
}

impl Nested {
    #[cfg(any(test, feature = "test-support"))]
    fn places_something(&self) -> bool {
        self.generator
            .as_ref()
            .is_some_and(Generator::places_something)
    }

    /// A nested feature draws from its parent's source, so its chain runs
    /// whether or not this build can place its leaf; skipping the chain would
    /// move every later draw of the same object. The outer stack machine is
    /// still walking its scratch, so a nested placement takes one from the
    /// run's pool.
    pub fn place<W: WorldGenVolume>(
        &self,
        run: &mut Run,
        region: &mut W,
        rng: &mut XoroshiroRandom,
        at: BlockPos,
        carries: &dyn Fn(u32) -> bool,
    ) -> bool {
        let generator = self.generator.as_ref();
        let mut scratch = run.scratch.placers.pop().unwrap_or_default();
        let placed = place(
            &self.placement,
            region,
            &mut scratch,
            at,
            rng,
            carries,
            &mut |region, rng, pos| {
                generator.is_some_and(|generator| generator.place(run, region, rng, pos, carries))
            },
        );
        run.scratch.placers.push(scratch);
        placed
    }
}

impl Generator {
    /// Whether anything this feature reaches can write a block.
    ///
    /// A container whose every entry is a shape this build has no code for
    /// still compiles — the entries keep their chains and their draws — so
    /// having a generator is not the same as placing something.
    #[cfg(any(test, feature = "test-support"))]
    pub fn places_something(&self) -> bool {
        let any = |nested: &[Nested]| nested.iter().any(Nested::places_something);
        match self {
            Generator::RandomSelector { features, default } => {
                features.iter().any(|(_, nested)| nested.places_something())
                    || default.places_something()
            }
            Generator::WeightedRandomSelector(features) => {
                features.iter().any(|(_, nested)| nested.places_something())
            }
            Generator::RandomBooleanSelector {
                feature_true,
                feature_false,
            } => feature_true.places_something() || feature_false.places_something(),
            Generator::Overlay(features) | Generator::Sequence(features) => any(features),
            Generator::CoralTree(nested) | Generator::CoralClaw(nested) => {
                nested.places_something()
            }
            // These write on their own account and only then run the nested
            // feature, so an entry this build cannot place does not empty them.
            Generator::RootSystem { .. }
            | Generator::VegetationPatch { .. }
            | Generator::SingleBlockPillar { .. } => true,
            Generator::NoOp => false,
            _ => true,
        }
    }

    /// Place this feature at `at`; true when it wrote anything.
    pub fn place<W: WorldGenVolume>(
        &self,
        run: &mut Run,
        region: &mut W,
        rng: &mut XoroshiroRandom,
        at: BlockPos,
        carries: &dyn Fn(u32) -> bool,
    ) -> bool {
        match self {
            Generator::Ore(ore) => place_modern_ore(ore, region, rng, at, &mut run.scratch.ore),
            Generator::Tree(tree) => {
                place_tree::<W>(tree, region, rng, &mut TreeRun { run, carries }, at)
            }
            Generator::RandomSelector { features, default } => {
                for (chance, nested) in features {
                    if rng.next_f32() < *chance {
                        return nested.place(run, region, rng, at, carries);
                    }
                }
                default.place(run, region, rng, at, carries)
            }
            Generator::WeightedRandomSelector(features) => {
                match pick_weighted_by(features, |(weight, _)| *weight, rng) {
                    Some((_, nested)) => nested.place(run, region, rng, at, carries),
                    None => false,
                }
            }
            Generator::RandomBooleanSelector {
                feature_true,
                feature_false,
            } => {
                let nested = if rng.next_bool() {
                    feature_true
                } else {
                    feature_false
                };
                nested.place(run, region, rng, at, carries)
            }
            Generator::FallenTree(tree) => {
                place_fallen_tree(tree, region, rng, &mut TreeRun { run, carries }, at)
            }
            Generator::RootSystem { config, feature } => {
                place_root_system(config, region, rng, at, &mut |region, rng, pos| {
                    feature.place(run, region, rng, pos, carries)
                })
            }
            Generator::SimpleBlock(config) => place_simple_block(config, region, rng, at),
            Generator::BlockColumn(config) => place_block_column(config, region, rng, at),
            Generator::BlockPile(config) => place_block_pile(config, region, rng, at),
            Generator::MultifaceGrowth(config) => place_multiface_growth(config, region, rng, at),
            Generator::Vines(vine) => place_vines(vine, region, at),
            Generator::RandomNeighborSpread(config) => {
                place_random_neighbor_spread(config, region, rng, at)
            }
            // Every entry runs; the feature placed if any did.
            Generator::Overlay(features) => features.iter().fold(false, |placed, nested| {
                nested.place(run, region, rng, at, carries) | placed
            }),
            // The first refusal ends the run, so later entries spend no draws.
            Generator::Sequence(features) => features
                .iter()
                .all(|nested| nested.place(run, region, rng, at, carries)),
            Generator::ScatteredOre(ore) => place_scattered_ore(ore, region, rng, at),
            Generator::VegetationPatch { config, feature } => {
                place_vegetation_patch(config, region, rng, at, &mut |region, rng, pos| {
                    feature.place(run, region, rng, pos, carries)
                })
            }
            Generator::Disk(config) => place_disk(config, region, rng, at),
            Generator::BlockBlob(config) => place_block_blob(config, region, rng, at),
            Generator::ReplaceBlobs(config) => place_replace_blobs(config, region, rng, at),
            Generator::Delta(config) => place_delta(config, region, rng, at),
            Generator::UnderwaterMagma(config) => place_underwater_magma(config, region, rng, at),
            Generator::BlueIce(config) => place_blue_ice(config, region, rng, at),
            Generator::FreezeTopLayer(config) => place_freeze_top_layer(config, region, at),
            Generator::Spring(config) => place_spring(config, region, rng, at),
            Generator::MonsterRoom(config) => {
                place_monster_room(config, region, rng, &mut run.entities, at)
            }
            Generator::BonusChest(config) => {
                place_bonus_chest(config, region, rng, &mut run.entities, at)
            }
            Generator::Lake(config) => place_lake(config, region, rng, at),
            Generator::Geode(config) => place_geode(config, region, rng, at),
            Generator::SpeleothemCluster(config) => {
                place_speleothem_cluster(config, region, rng, at)
            }
            Generator::LargeDripstone(config) => place_large_dripstone(config, region, rng, at),
            Generator::Bamboo(config) => place_bamboo(config, region, rng, at),
            Generator::ChorusPlant(config) => place_chorus_plant(config, region, rng, at),
            Generator::HugeFungus(config) => place_huge_fungus(config, region, rng, at),
            Generator::HugeMushroom(config) => place_huge_mushroom(config, region, rng, at),
            Generator::Iceberg(config) => place_iceberg(config, region, rng, at),
            Generator::Spike(config) => place_spike(config, region, rng, at),
            Generator::Speleothem(config) => place_speleothem(config, region, rng, at),
            Generator::CoralTree(feature) => {
                place_coral_tree(region, rng, at, &mut |region, rng, pos| {
                    feature.place(run, region, rng, pos, carries)
                })
            }
            Generator::CoralClaw(feature) => {
                place_coral_claw(region, rng, at, &mut |region, rng, pos| {
                    feature.place(run, region, rng, pos, carries)
                })
            }
            Generator::SingleBlockPillar { config, cap } => {
                place_single_block_pillar(config, region, rng, at, &mut |region, rng, pos| {
                    cap.as_ref()
                        .is_some_and(|cap| cap.place(run, region, rng, pos, carries))
                })
            }
            Generator::ProjectedPatchySquare(config) => {
                place_projected_random_patchy_square(config, region, rng, at)
            }
            Generator::SculkPatch(config) => place_sculk_patch(config, region, rng, at),
            Generator::SteppedColumnCluster(config) => {
                place_stepped_column_cluster(config, region, rng, at)
            }
            Generator::NoOp => true,
            Generator::EndPlatform(config) => place_end_platform(config, region, at),
            Generator::VoidStartPlatform(config) => place_void_start_platform(config, region, at),
            Generator::EndPodium(config) => place_end_podium(config, region, at),
            Generator::EndGateway(config) => {
                place_end_gateway(config, region, &mut run.entities, at)
            }
            Generator::EndIsland(config) => place_end_island(config, region, rng, at),
            // An end crystal is an entity, and worldgen has no entity channel;
            // its yaw draw still happens, so nothing after it moves.
            Generator::EndSpikes(config) => place_end_spike(config, region, rng, at),
            Generator::FillLayer(config) => place_fill_layer(config, region, at),
            Generator::BetaPopulate(populate) => {
                apply_beta_ores_in(
                    region,
                    at.x.div_euclid(16),
                    at.z.div_euclid(16),
                    populate.world_seed,
                    &populate.ids,
                );
                true
            }
            Generator::ReplaceSingleBlock(config) => {
                place_replace_single_block(config, region, rng, at)
            }
            Generator::Template(config) => {
                let Some((_, template, rotations)) =
                    pick_weighted_by(&config.entries, |(weight, _, _)| *weight, rng)
                else {
                    return false;
                };
                let rotation = rotations[rng.next_i32_bound(rotations.len() as i32) as usize];
                let half = |axis: usize| i32::from(template.size[axis]) / 2;
                let position = *at
                    + rotation.rotate(Direction::West).normal() * half(0)
                    + rotation.rotate(Direction::North).normal() * half(2);
                if template.palettes.is_empty() {
                    return false;
                }
                let palette = rng.next_i32_bound(template.palettes.len() as i32) as usize;
                place_template(
                    &Placement {
                        template,
                        jigsaws: &[],
                        palette,
                        position,
                        reference: position,
                        rotation,
                        mirror: Mirror::None,
                        pivot: IVec3::ZERO,
                        random: SettingsRandom::Stream,
                        clip: None,
                        chain: &config.chain,
                        waterlog: true,
                        place_entities: false,
                    },
                    region,
                    rng,
                    &mut run.entities,
                    &mut run.spawns,
                )
            }
            Generator::Fossil(config) => {
                place_fossil(config, region, rng, at, &mut run.entities, &mut run.spawns)
            }
        }
    }
}

fn place_fossil<W: WorldGenVolume>(
    config: &CompiledFossil,
    region: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
) -> bool {
    let rotation = Rotation::ALL[rng.next_i32_bound(4) as usize];
    let (fossil, overlay) = &config.pairs[rng.next_i32_bound(config.pairs.len() as i32) as usize];
    let extent = region.extent();
    let chunk_min = IVec3::new(
        at.x.div_euclid(16) * 16,
        extent.min_y,
        at.z.div_euclid(16) * 16,
    );
    let clip = BoundingBox {
        min: (chunk_min - IVec3::new(16, 0, 16)).into(),
        max: (chunk_min + IVec3::new(31, extent.depth - 1, 31)).into(),
    };
    let [size_x, _, size_z] = fossil.size.map(i32::from);
    let (span_x, span_z) = match rotation {
        Rotation::Clockwise90 | Rotation::Counterclockwise90 => (size_z, size_x),
        _ => (size_x, size_z),
    };
    let low = *at - IVec3::new(span_x / 2, 0, span_z / 2);
    let mut lowest_surface = at.y;
    for x in 0..span_x {
        for z in 0..span_z {
            lowest_surface = lowest_surface.min(region.height(
                HeightmapName::OceanFloorWg,
                low.x + x,
                low.z + z,
            ));
        }
    }
    let target_y = (lowest_surface - 15 - rng.next_i32_bound(10)).max(extent.min_y + 10);
    let target =
        zero_position_with_transform(low.with_y(target_y), Mirror::None, rotation, size_x, size_z);
    let world = region.world();
    let empty_corners = bounding_box(fossil.size, target, rotation, Mirror::None, IVec3::ZERO)
        .corners()
        .into_iter()
        .filter(|&corner| {
            let state = region.get(corner).0 as usize;
            world.air_states.contains(state)
                || world.lava_states.contains(state)
                || world.water_states.contains(state)
        })
        .count();
    if empty_corners > config.max_empty_corners {
        return false;
    }
    for (template, chain) in [
        (fossil, &config.fossil_chain),
        (overlay, &config.overlay_chain),
    ] {
        if template.palettes.is_empty() {
            continue;
        }
        let palette = rng.next_i32_bound(template.palettes.len() as i32) as usize;
        place_template(
            &Placement {
                template,
                jigsaws: &[],
                palette,
                position: target,
                reference: target,
                rotation,
                mirror: Mirror::None,
                pivot: IVec3::ZERO,
                random: SettingsRandom::Stream,
                clip: Some(clip),
                chain,
                waterlog: true,
                place_entities: false,
            },
            region,
            rng,
            entities,
            spawns,
        );
    }
    true
}

fn compile_elements(
    frozen: &FrozenStructures,
    trees: &Arc<TreeTables>,
    resolver: &Resolver<'_>,
    corpus: &LoadedFeatures,
) -> Compiled<Vec<CompiledElement>> {
    let mut chains: Vec<(
        (&Holder<StructureProcessorList>, ChainKind),
        Arc<CompiledChain>,
    )> = Vec::new();
    frozen
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let within =
                |error: FeatureCompileError| error.within(format!("template pool element {index}"));
            Ok(match element {
                FrozenElement::Single {
                    template,
                    legacy,
                    processors,
                    projection,
                    liquid_settings,
                } => {
                    let kind = ChainKind::Piece {
                        projection: *projection,
                        legacy: *legacy,
                    };
                    let key = (processors, kind);
                    let chain = match chains.iter().find(|(k, _)| *k == key) {
                        Some((_, chain)) => Some(Arc::clone(chain)),
                        None => {
                            let list = match processors {
                                Holder::Reference(id) => {
                                    corpus.processor_lists.get(id).ok_or_else(|| {
                                        within(FeatureCompileError::UnknownProcessorList(
                                            id.clone(),
                                        ))
                                    })?
                                }
                                Holder::Inline(list) => list,
                            };
                            match compile_chain(
                                processor_list(list),
                                kind,
                                resolver,
                                resolver.world_seed,
                            ) {
                                Ok(chain) => {
                                    let chain = Arc::new(chain);
                                    chains.push((key, Arc::clone(&chain)));
                                    Some(chain)
                                }
                                Err(error) if error.is_unsupported() => {
                                    tracing::warn!(
                                        element = index,
                                        %error,
                                        "the pool element places nothing"
                                    );
                                    None
                                }
                                Err(error) => return Err(within(error)),
                            }
                        }
                    };
                    CompiledElement::Single {
                        template: Arc::clone(&frozen.templates[template.0 as usize]),
                        manifest: Arc::clone(&frozen.manifests[template.0 as usize]),
                        chain,
                        liquid: *liquid_settings,
                    }
                }
                FrozenElement::List { elements, .. } => CompiledElement::List(elements.clone()),
                FrozenElement::Feature { feature, .. } => CompiledElement::Feature(
                    compile_nested(feature, trees, resolver, corpus).map_err(within)?,
                ),
                FrozenElement::Empty => CompiledElement::Empty,
            })
        })
        .collect()
}

type Compiled<T> = Result<T, FeatureCompileError>;

fn compile_structures(
    frozen: &FrozenStructures,
    resolver: &Resolver<'_>,
) -> Compiled<Vec<Option<CompiledStructure>>> {
    frozen
        .structures
        .iter()
        .map(|structure| {
            Ok(match &structure.kind {
                StructureKind::DesertPyramid => Some(CompiledStructure::DesertPyramid(Box::new(
                    DesertPyramidBlocks::compile(resolver, &resolver.world, resolver.world_seed)
                        .map_err(|error| error.within(&structure.id))?,
                ))),
                StructureKind::JungleTemple => Some(CompiledStructure::JungleTemple(Box::new(
                    JungleTempleBlocks::compile(resolver, &resolver.world)
                        .map_err(|error| error.within(&structure.id))?,
                ))),
                StructureKind::BuriedTreasure => Some(CompiledStructure::BuriedTreasure(Box::new(
                    BuriedTreasureBlocks::compile(resolver, &resolver.world)
                        .map_err(|error| error.within(&structure.id))?,
                ))),
                StructureKind::Fortress => Some(CompiledStructure::Fortress(Box::new(
                    FortressBlocks::compile(resolver, &resolver.world)
                        .map_err(|error| error.within(&structure.id))?,
                ))),
                StructureKind::Shipwreck { .. } => Some(CompiledStructure::Shipwreck(
                    ignore_structure_and_air(resolver)
                        .map_err(|error| error.within(&structure.id))?,
                )),
                StructureKind::OceanRuin(config) => Some(CompiledStructure::OceanRuin(Box::new(
                    OceanRuinBlocks::compile(resolver, config.biome_temp, resolver.world_seed)
                        .map_err(|error| error.within(&structure.id))?,
                ))),
                StructureKind::RuinedPortal { setups, .. } => {
                    Some(CompiledStructure::RuinedPortal(Box::new(
                        RuinedPortalBlocks::compile(setups, resolver, resolver.world_seed)
                            .map_err(|error| error.within(&structure.id))?,
                    )))
                }
                StructureKind::OceanMonument { .. } => {
                    Some(CompiledStructure::OceanMonument(Box::new(
                        OceanMonumentBlocks::compile(resolver, &resolver.world)
                            .map_err(|error| error.within(&structure.id))?,
                    )))
                }
                StructureKind::Mineshaft {
                    mineshaft_type,
                    blocking,
                } => Some(CompiledStructure::Mineshaft(Box::new(
                    MineshaftBlocks::compile(
                        resolver,
                        &resolver.world,
                        *mineshaft_type,
                        Arc::clone(blocking),
                    )
                    .map_err(|error| error.within(&structure.id))?,
                ))),
                StructureKind::Igloo => Some(CompiledStructure::Igloo(Box::new(
                    IglooBlocks::compile(resolver).map_err(|error| error.within(&structure.id))?,
                ))),
                StructureKind::NetherFossil { .. } => {
                    Some(CompiledStructure::NetherFossil(Box::new(
                        NetherFossilBlocks::compile(resolver, resolver.world_seed)
                            .map_err(|error| error.within(&structure.id))?,
                    )))
                }
                _ => None,
            })
        })
        .collect()
}

fn compile_generator(
    feature: &Feature,
    trees: &Arc<TreeTables>,
    resolver: &Resolver<'_>,
    corpus: &LoadedFeatures,
) -> Compiled<Generator> {
    Ok(match feature {
        Feature::Ore {
            targets,
            size,
            discard_chance_on_air_exposure,
        }
        | Feature::ScatteredOre {
            targets,
            size,
            discard_chance_on_air_exposure,
        } => {
            let ore = compile_ore(
                targets,
                size.0,
                discard_chance_on_air_exposure.0 as f32,
                resolver,
            )?;
            if matches!(feature, Feature::Ore { .. }) {
                Generator::Ore(ore)
            } else {
                Generator::ScatteredOre(ore)
            }
        }
        Feature::Tree(config) => Generator::Tree(Box::new(compile_tree(config, trees, resolver)?)),
        Feature::RandomSelector { features, default } => {
            let entries = features
                .iter()
                .map(|entry: &WeightedPlacedFeature| {
                    Ok((
                        entry.chance.0 as f32,
                        compile_nested(&entry.feature, trees, resolver, corpus)?,
                    ))
                })
                .collect::<Compiled<Vec<_>>>()?;
            Generator::RandomSelector {
                features: entries,
                default: Box::new(compile_nested(default, trees, resolver, corpus)?),
            }
        }
        Feature::SimpleRandomSelector { features } => {
            let entries = holders(features)?
                .iter()
                .map(|holder| Ok((1, compile_nested(holder, trees, resolver, corpus)?)))
                .collect::<Compiled<Vec<_>>>()?;
            Generator::WeightedRandomSelector(entries)
        }
        Feature::RandomBooleanSelector {
            feature_true,
            feature_false,
        } => Generator::RandomBooleanSelector {
            feature_true: Box::new(compile_nested(feature_true, trees, resolver, corpus)?),
            feature_false: Box::new(compile_nested(feature_false, trees, resolver, corpus)?),
        },
        Feature::FallenTree {
            trunk_provider,
            log_length,
            stump_decorators,
            log_decorators,
        } => Generator::FallenTree(Box::new(CompiledFallenTree {
            trunk_provider: compile_provider(trunk_provider, resolver)?,
            log_length: log_length.clone(),
            stump_decorators: stump_decorators
                .iter()
                .map(|d| compile_decorator(d, resolver))
                .collect::<Compiled<_>>()?,
            log_decorators: log_decorators
                .iter()
                .map(|d| compile_decorator(d, resolver))
                .collect::<Compiled<_>>()?,
            tables: trees.clone(),
        })),
        Feature::RootSystem {
            feature: nested,
            required_vertical_space_for_tree,
            level_test_distance,
            max_level_deviation,
            root_radius,
            root_replaceable,
            root_state_provider,
            root_placement_attempts,
            root_column_max_height,
            hanging_root_radius,
            hanging_roots_vertical_span,
            hanging_root_state_provider,
            hanging_root_placement_attempts,
            allowed_vertical_water_for_tree,
            allowed_tree_position,
        } => {
            let hanging = compile_provider(hanging_root_state_provider, resolver)?;
            // Only a constant provider names one block, so only it can be answered;
            // anything else survives, which is the default `canSurvive`.
            let hanging_survive = match &hanging {
                StateProvider::Simple(state) => {
                    let index = resolver
                        .blocks
                        .block_index(mcrs_minecraft_registry::BlockStateId(state.0));
                    trees.survive.get(&index).cloned()
                }
                _ => None,
            };
            let config = CompiledRootSystem {
                required_vertical_space_for_tree: required_vertical_space_for_tree.0,
                level_test_distance: level_test_distance.0,
                max_level_deviation: max_level_deviation.0,
                root_radius: root_radius.0,
                root_replaceable: resolver.mask(StateQuery::Blocks(root_replaceable))?,
                root_state_provider: compile_provider(root_state_provider, resolver)?,
                root_placement_attempts: root_placement_attempts.0,
                root_column_max_height: root_column_max_height.0,
                hanging_root_radius: hanging_root_radius.0,
                hanging_roots_vertical_span: hanging_roots_vertical_span.0,
                hanging_root_state_provider: hanging,
                hanging_root_placement_attempts: hanging_root_placement_attempts.0,
                allowed_vertical_water_for_tree: allowed_vertical_water_for_tree.0,
                allowed_tree_position: compile_predicate(allowed_tree_position, resolver)?,
                hanging_survive,
            };
            Generator::RootSystem {
                config: Box::new(config),
                feature: Box::new(compile_nested(nested, trees, resolver, corpus)?),
            }
        }
        Feature::WeightedRandomSelector { features } => {
            let entries = features
                .iter()
                .filter(|entry| entry.weight.0 > 0)
                .map(|entry| {
                    Ok((
                        entry.weight.0,
                        compile_nested(&entry.data, trees, resolver, corpus)?,
                    ))
                })
                .collect::<Compiled<Vec<_>>>()?;
            Generator::WeightedRandomSelector(entries)
        }
        Feature::SimpleBlock {
            to_place,
            schedule_tick,
        } => {
            if *schedule_tick {
                // The reference schedules a block tick so the state settles on
                // the tick after generation — an eyeblossom opens or closes
                // with the day. Nothing here schedules, so it stays as it went
                // in. Named rather than dropped, so the gap is one grep away.
                tracing::warn!(
                    to_place = ?to_place,
                    "a simple_block asks for a scheduled tick and this build has no block tick scheduler; the state is left as placed"
                );
            }
            Generator::SimpleBlock(Box::new(CompiledSimpleBlock {
                to_place: compile_provider(to_place, resolver)?,
                tables: resolver.tables.clone(),
            }))
        }
        Feature::BlockColumn {
            layers,
            direction,
            allowed_placement,
            prioritize_tip,
        } => Generator::BlockColumn(Box::new(CompiledBlockColumn {
            layers: layers
                .iter()
                .map(|layer| {
                    Ok(ColumnLayer {
                        height: layer.height.clone(),
                        provider: compile_provider(&layer.provider, resolver)?,
                    })
                })
                .collect::<Compiled<_>>()?,
            direction: *direction,
            allowed_placement: compile_predicate(allowed_placement, resolver)?,
            prioritize_tip: *prioritize_tip,
        })),
        Feature::BlockPile { state_provider } => Generator::BlockPile(CompiledBlockPile {
            state_provider: compile_provider(state_provider, resolver)?,
            dirt_path: resolver.block_mask("minecraft:dirt_path")?,
        }),
        Feature::MultifaceGrowth {
            block,
            can_place_on_floor,
            can_place_on_ceiling,
            can_place_on_wall,
            chance_of_spreading,
            can_be_placed_on,
            ..
        } => Generator::MultifaceGrowth(Box::new(CompiledMultifaceGrowth {
            states: missing(multiface_states(resolver.blocks, block.as_str()), block)?,
            valid_directions: valid_directions(
                *can_place_on_ceiling,
                *can_place_on_floor,
                *can_place_on_wall,
            ),
            chance_of_spreading: chance_of_spreading.0 as f32,
            can_be_placed_on: resolver.mask(StateQuery::Blocks(can_be_placed_on))?,
        })),
        Feature::Vines => Generator::Vines(missing(resolver.tables.vine, "minecraft:vine")?),
        Feature::RandomNeighborSpread {
            block,
            accepted_neighbors,
            can_replace,
            attempts,
            xz_offset,
            y_offset,
        } => Generator::RandomNeighborSpread(Box::new(CompiledNeighborSpread {
            block: compile_provider(block, resolver)?,
            accepted_neighbors: resolver.mask(StateQuery::Blocks(accepted_neighbors))?,
            can_replace: compile_predicate(can_replace, resolver)?,
            attempts: attempts.0.clone(),
            xz_offset: xz_offset.0.clone(),
            y_offset: y_offset.0.clone(),
        })),
        Feature::Overlay { features } => Generator::Overlay(
            holders(features)?
                .iter()
                .map(|holder| compile_nested(holder, trees, resolver, corpus))
                .collect::<Compiled<_>>()?,
        ),
        Feature::Sequence { features } => {
            let entries: Vec<Nested> = holders(features)?
                .iter()
                .map(|holder| compile_nested(holder, trees, resolver, corpus))
                .collect::<Compiled<_>>()?;
            // The first refusal ends the run, so an entry this build cannot
            // place would swallow every later entry's draws.
            if entries.iter().any(|nested| nested.generator.is_none()) {
                return Err(FeatureCompileError::Unsupported(
                    "a sequence entry this build cannot place".to_owned(),
                ));
            }
            Generator::Sequence(entries)
        }
        Feature::VegetationPatch(config) => {
            compile_vegetation_patch(config, false, trees, resolver, corpus)?
        }
        Feature::WaterloggedVegetationPatch(config) => {
            compile_vegetation_patch(config, true, trees, resolver, corpus)?
        }
        Feature::Disk {
            state_provider,
            target,
            radius,
            half_height,
        } => Generator::Disk(Box::new(CompiledDisk {
            state_provider: compile_provider(state_provider, resolver)?,
            target: compile_predicate(target, resolver)?,
            radius: radius.0.clone(),
            half_height: half_height.0,
        })),
        Feature::BlockBlob {
            state,
            can_place_on,
        } => Generator::BlockBlob(CompiledBlockBlob {
            state: resolver.resolve(state)?,
            can_place_on: compile_predicate(can_place_on, resolver)?,
        }),
        Feature::NetherrackReplaceBlobs {
            target,
            state,
            radius,
        } => Generator::ReplaceBlobs(CompiledReplaceBlobs {
            target: resolver.block_mask(target.name.as_str())?,
            state: resolver.resolve(state)?,
            radius: radius.0.clone(),
        }),
        Feature::Delta {
            contents,
            rim,
            size,
            rim_size,
        } => Generator::Delta(Box::new(CompiledDelta {
            contents: resolver.resolve(contents)?,
            contents_block: resolver.block_mask(contents.name.as_str())?,
            rim: resolver.resolve(rim)?,
            size: size.0.clone(),
            rim_size: rim_size.0.clone(),
            cannot_replace: resolver.blocks_mask(DELTA_CANNOT_REPLACE)?,
        })),
        Feature::UnderwaterMagma {
            floor_search_range,
            placement_radius_around_floor,
            placement_probability_per_valid_position,
        } => Generator::UnderwaterMagma(CompiledUnderwaterMagma {
            floor_search_range: floor_search_range.0,
            placement_radius_around_floor: placement_radius_around_floor.0,
            placement_probability_per_valid_position: placement_probability_per_valid_position.0
                as f32,
            magma: resolver.default_state("minecraft:magma_block")?,
        }),
        Feature::BlueIce => Generator::BlueIce(CompiledBlueIce {
            blue_ice: resolver.default_state("minecraft:blue_ice")?,
            packed_ice: resolver.default_state("minecraft:packed_ice")?,
            ice: resolver.default_state("minecraft:ice")?,
        }),
        Feature::FreezeTopLayer => {
            let snow = resolver.block("minecraft:snow")?;
            Generator::FreezeTopLayer(Box::new(CompiledFreezeTopLayer {
                biomes: resolver.climate.to_vec(),
                ice: resolver.default_state("minecraft:ice")?,
                snow: VoxelId::from(snow.default_state_id.0),
                snow_layers_8: VoxelId::from(set(snow, snow.default_state_id, "layers", "8")?.0),
                snow_states: resolver.block_mask("minecraft:snow")?,
                cannot_support_snow: resolver.tag_mask("minecraft:cannot_support_snow_layer")?,
                support_override_snow: resolver
                    .tag_mask("minecraft:support_override_snow_layer")?,
                tables: resolver.tables.clone(),
            }))
        }
        Feature::Spring {
            state,
            requires_block_below,
            rock_count,
            hole_count,
            valid_blocks,
        } => Generator::Spring(CompiledSpring {
            state: missing(spring_state(state, resolver), state_named(state))?,
            requires_block_below: *requires_block_below,
            rock_count: rock_count.0,
            hole_count: hole_count.0,
            valid_blocks: resolver.mask(StateQuery::Blocks(valid_blocks))?,
        }),
        Feature::MonsterRoom => Generator::MonsterRoom(Box::new(CompiledMonsterRoom {
            cobblestone: resolver.default_state("minecraft:cobblestone")?,
            mossy_cobblestone: resolver.default_state("minecraft:mossy_cobblestone")?,
            spawner: resolver.default_state("minecraft:spawner")?,
            chest_facing: missing(
                horizontal_facings(resolver.blocks, "minecraft:chest"),
                "minecraft:chest",
            )?,
            chest_states: resolver.block_mask("minecraft:chest")?,
            spawner_states: resolver.block_mask("minecraft:spawner")?,
            cannot_replace: resolver.tag_mask("minecraft:features_cannot_replace")?,
        })),
        Feature::BonusChest => Generator::BonusChest(CompiledBonusChest {
            chest: resolver.default_state("minecraft:chest")?,
            torch: resolver.default_state("minecraft:torch")?,
        }),
        Feature::Lake {
            fluid,
            barrier,
            can_place_feature,
            can_replace_with_air_or_fluid,
            can_replace_with_barrier,
        } => Generator::Lake(Box::new(CompiledLake {
            fluid: compile_provider(fluid, resolver)?,
            barrier: compile_provider(barrier, resolver)?,
            can_place_feature: compile_predicate(can_place_feature, resolver)?,
            can_replace_with_air_or_fluid: compile_predicate(
                can_replace_with_air_or_fluid,
                resolver,
            )?,
            can_replace_with_barrier: compile_predicate(can_replace_with_barrier, resolver)?,
            ice: resolver.default_state("minecraft:ice")?,
            freezing_biomes: resolver.freezing_biomes(),
        })),
        Feature::Geode {
            blocks,
            layers,
            crack,
            use_potential_placements_chance,
            use_alternate_layer0_chance,
            placements_require_layer0_alternate,
            outer_wall_distance,
            distribution_points,
            point_offset,
            min_gen_offset,
            max_gen_offset,
            noise_multiplier,
            invalid_blocks_threshold,
        } => {
            let outer_wall_distance = outer_wall_distance
                .clone()
                .unwrap_or(IntProviderRef::uniform(4, 5));
            Generator::Geode(Box::new(CompiledGeode {
                filling: compile_provider(&blocks.filling_provider, resolver)?,
                inner_layer: compile_provider(&blocks.inner_layer_provider, resolver)?,
                alternate_inner_layer: compile_provider(
                    &blocks.alternate_inner_layer_provider,
                    resolver,
                )?,
                middle_layer: compile_provider(&blocks.middle_layer_provider, resolver)?,
                outer_layer: compile_provider(&blocks.outer_layer_provider, resolver)?,
                inner_placements: blocks
                    .inner_placements
                    .iter()
                    .map(|state| {
                        missing(GeodeCrystal::resolve(state, resolver), state_named(state))
                    })
                    .collect::<Compiled<_>>()?,
                cannot_replace: resolver.mask(StateQuery::Blocks(&blocks.cannot_replace))?,
                invalid_blocks: resolver.mask(StateQuery::Blocks(&blocks.invalid_blocks))?,
                cluster_growable: union_masks(&[
                    &resolver.world.air_states,
                    &resolver.world.water_source,
                ]),
                use_potential_placements_chance: use_potential_placements_chance.0,
                use_alternate_layer0_chance: use_alternate_layer0_chance.0,
                placements_require_layer0_alternate: *placements_require_layer0_alternate,
                outer_wall_distance_max: outer_wall_distance.bounds().1,
                outer_wall_distance,
                distribution_points: distribution_points
                    .clone()
                    .unwrap_or(IntProviderRef::uniform(3, 4)),
                point_offset: point_offset
                    .clone()
                    .unwrap_or(IntProviderRef::uniform(1, 2)),
                min_gen_offset: min_gen_offset.0,
                max_gen_offset: max_gen_offset.0,
                noise_multiplier: noise_multiplier.0,
                invalid_blocks_threshold: *invalid_blocks_threshold,
                filling_thickness: layers.filling.0,
                inner_layer_thickness: layers.inner_layer.0,
                middle_layer_thickness: layers.middle_layer.0,
                outer_layer_thickness: layers.outer_layer.0,
                base_crack_size: crack.base_crack_size.0,
                generate_crack_chance: crack.generate_crack_chance.0,
                crack_point_offset: crack.crack_point_offset.0,
                noise: Arc::new(mcrs_minecraft_worldgen_noise::normal::create_parity(
                    -4,
                    &[1.0],
                    &mut LegacyRandom::new(resolver.world_seed as u64),
                )),
            }))
        }
        Feature::SpeleothemCluster {
            base_block,
            pointed_block,
            replaceable_blocks,
            floor_to_ceiling_search_range,
            height,
            radius,
            max_stalagmite_stalactite_height_diff,
            height_deviation,
            speleothem_block_layer_thickness,
            density,
            wetness,
            chance_of_speleothem_at_max_distance_from_center,
            max_distance_from_edge_affecting_chance_of_speleothem,
            max_distance_from_center_affecting_height_bias,
        } => {
            let base_block_states = resolver.block_mask(base_block.name.as_str())?;
            let replaceable = resolver.mask(StateQuery::Blocks(replaceable_blocks))?;
            Generator::SpeleothemCluster(Box::new(CompiledSpeleothemCluster {
                base_block: resolver.resolve(base_block)?,
                base_or_replaceable: union_masks(&[&base_block_states, &replaceable]),
                base_block_states,
                pointed: missing(
                    PointedStates::resolve(pointed_block, resolver),
                    state_named(pointed_block),
                )?,
                pointed_block_states: resolver.block_mask(pointed_block.name.as_str())?,
                replaceable_blocks: replaceable,
                floor_to_ceiling_search_range: floor_to_ceiling_search_range.0,
                height: height.0.clone(),
                radius: radius.0.clone(),
                max_stalagmite_stalactite_height_diff: max_stalagmite_stalactite_height_diff.0,
                height_deviation: height_deviation.0,
                speleothem_block_layer_thickness: speleothem_block_layer_thickness.0.clone(),
                density: *density,
                wetness: *wetness,
                chance_of_speleothem_at_max_distance_from_center:
                    chance_of_speleothem_at_max_distance_from_center.0 as f32,
                max_distance_from_edge_affecting_chance_of_speleothem:
                    max_distance_from_edge_affecting_chance_of_speleothem.0,
                max_distance_from_center_affecting_height_bias:
                    max_distance_from_center_affecting_height_bias.0,
                base_stone_overworld: resolver.tag_mask("minecraft:base_stone_overworld")?,
            }))
        }
        Feature::LargeDripstone {
            replaceable_blocks,
            floor_to_ceiling_search_range,
            column_radius,
            height_scale,
            max_column_radius_to_cave_height_ratio,
            stalactite_bluntness,
            stalagmite_bluntness,
            wind_speed,
            min_radius_for_wind,
            min_bluntness_for_wind,
        } => {
            let replaceable = resolver.mask(StateQuery::Blocks(replaceable_blocks))?;
            let dripstone = resolver.block_mask("minecraft:dripstone_block")?;
            let (column_radius_min, column_radius_max) = column_radius.bounds();
            Generator::LargeDripstone(Box::new(CompiledLargeDripstone {
                dripstone: resolver.default_state("minecraft:dripstone_block")?,
                column_edge: union_masks(&[&dripstone, &replaceable, &resolver.world.lava_states]),
                floor_to_ceiling_search_range: floor_to_ceiling_search_range.0,
                column_radius_min,
                column_radius_max,
                height_scale: *height_scale,
                max_column_radius_to_cave_height_ratio: max_column_radius_to_cave_height_ratio.0
                    as f32,
                stalactite_bluntness: *stalactite_bluntness,
                stalagmite_bluntness: *stalagmite_bluntness,
                wind_speed: *wind_speed,
                min_radius_for_wind: min_radius_for_wind.0,
                min_bluntness_for_wind: min_bluntness_for_wind.0 as f32,
                base_stone_overworld: resolver.tag_mask("minecraft:base_stone_overworld")?,
            }))
        }
        Feature::Bamboo { probability } => {
            let bamboo = resolver.block("minecraft:bamboo")?;
            let stalk = |leaves: &str, stage: &str| -> Compiled<VoxelId> {
                let id = set(bamboo, bamboo.default_state_id, "age", "1")?;
                let id = set(bamboo, id, "leaves", leaves)?;
                let id = set(bamboo, id, "stage", stage)?;
                Ok(VoxelId::from(id.0))
            };
            Generator::Bamboo(CompiledBamboo {
                probability: probability.0 as f32,
                supports_bamboo: resolver.tag_mask("minecraft:supports_bamboo")?,
                beneath_podzol_replaceable: resolver
                    .tag_mask("minecraft:beneath_bamboo_podzol_replaceable")?,
                podzol: resolver.default_state("minecraft:podzol")?,
                trunk: stalk("none", "0")?,
                final_large: stalk("large", "1")?,
                top_large: stalk("large", "0")?,
                top_small: stalk("small", "0")?,
            })
        }
        Feature::ChorusPlant => {
            let plant = resolver.block("minecraft:chorus_plant")?;
            let flower = resolver.block("minecraft:chorus_flower")?;
            // The index is the six faces as bits, `down` highest, in
            // `connection_index` order.
            let mut plant_by_connections = [VoxelId(0); 64];
            for (bits, slot) in plant_by_connections.iter_mut().enumerate() {
                let faces = ["down", "up", "north", "east", "south", "west"];
                *slot = missing(with_bits(plant, &faces, bits), &plant.identifier)?;
            }
            Generator::ChorusPlant(Box::new(CompiledChorusPlant {
                supports: resolver.tag_mask("minecraft:supports_chorus_plant")?,
                plant_or_flower: resolver
                    .blocks_mask(&["minecraft:chorus_plant", "minecraft:chorus_flower"])?,
                plant_by_connections,
                flower_age5: VoxelId::from(set(flower, flower.default_state_id, "age", "5")?.0),
            }))
        }
        Feature::HugeFungus {
            valid_base_block,
            stem_state,
            hat_state,
            decor_state,
            replaceable_blocks,
            planted,
        } => {
            let weeping = resolver.block("minecraft:weeping_vines")?;
            Generator::HugeFungus(Box::new(CompiledHugeFungus {
                valid_base: resolver.block_mask(valid_base_block.name.as_str())?,
                stem_state: resolver.resolve(stem_state)?,
                hat_block: resolver.block_mask(hat_state.name.as_str())?,
                place_vines: hat_state.name.as_str() == "minecraft:nether_wart_block",
                hat_state: resolver.resolve(hat_state)?,
                decor_state: resolver.resolve(decor_state)?,
                replaceable_blocks: compile_predicate(replaceable_blocks, resolver)?,
                planted: *planted,
                weeping_vines_plant: resolver.default_state("minecraft:weeping_vines_plant")?,
                weeping_vines_by_age: [
                    VoxelId::from(set(weeping, weeping.default_state_id, "age", "23")?.0),
                    VoxelId::from(set(weeping, weeping.default_state_id, "age", "24")?.0),
                    VoxelId::from(set(weeping, weeping.default_state_id, "age", "25")?.0),
                ],
            }))
        }
        Feature::HugeBrownMushroom {
            cap_provider,
            stem_provider,
            foliage_radius,
            can_place_on,
        }
        | Feature::HugeRedMushroom {
            cap_provider,
            stem_provider,
            foliage_radius,
            can_place_on,
        } => Generator::HugeMushroom(Box::new(CompiledHugeMushroom {
            cap: if matches!(feature, Feature::HugeRedMushroom { .. }) {
                MushroomCap::Red
            } else {
                MushroomCap::Brown
            },
            cap_provider: compile_provider(cap_provider, resolver)?,
            stem_provider: compile_provider(stem_provider, resolver)?,
            foliage_radius: foliage_radius.0,
            can_place_on: compile_predicate(can_place_on, resolver)?,
            leaves: resolver.tag_mask("minecraft:leaves")?,
            replaceable_by_mushrooms: resolver.tag_mask("minecraft:replaceable_by_mushrooms")?,
            tables: resolver.tables.clone(),
        })),
        Feature::Iceberg { state } => Generator::Iceberg(CompiledIceberg {
            state: resolver.resolve(state)?,
            ice_mask: resolver.block_mask("minecraft:ice")?,
            snow_block: resolver.default_state("minecraft:snow_block")?,
            snow_block_mask: resolver.block_mask("minecraft:snow_block")?,
            snow_layer_mask: resolver.block_mask("minecraft:snow")?,
            iceberg_mask: resolver.blocks_mask(&[
                "minecraft:packed_ice",
                "minecraft:snow_block",
                "minecraft:blue_ice",
            ])?,
        }),
        Feature::Spike {
            state,
            can_place_on,
            can_replace,
        } => Generator::Spike(CompiledSpike {
            state: resolver.resolve(state)?,
            can_place_on: compile_predicate(can_place_on, resolver)?,
            can_replace: compile_predicate(can_replace, resolver)?,
        }),
        Feature::EndPlatform => Generator::EndPlatform(CompiledEndPlatform {
            obsidian: resolver.default_state("minecraft:obsidian")?,
        }),
        Feature::VoidStartPlatform => Generator::VoidStartPlatform(CompiledVoidStartPlatform {
            stone: resolver.default_state("minecraft:stone")?,
            cobblestone: resolver.default_state("minecraft:cobblestone")?,
        }),
        Feature::EndPodium { active } => Generator::EndPodium(CompiledEndPodium {
            active: *active,
            bedrock: resolver.default_state("minecraft:bedrock")?,
            end_stone: resolver.default_state("minecraft:end_stone")?,
            end_portal: resolver.default_state("minecraft:end_portal")?,
            wall_torch: missing(
                horizontal_facings(resolver.blocks, "minecraft:wall_torch"),
                "minecraft:wall_torch",
            )?,
        }),
        Feature::EndGateway { exit, exact } => Generator::EndGateway(CompiledEndGateway {
            exit: *exit,
            exact: *exact,
            gateway: resolver.default_state("minecraft:end_gateway")?,
            bedrock: resolver.default_state("minecraft:bedrock")?,
        }),
        Feature::EndIsland => Generator::EndIsland(CompiledEndIsland {
            end_stone: resolver.default_state("minecraft:end_stone")?,
        }),
        Feature::EndSpikes { spikes, .. } => {
            let bars = resolver.block("minecraft:iron_bars")?;
            // The index is the four sides as bits, `north` highest, in
            // `iron_bars_index` order.
            let mut iron_bars = [VoxelId(0); 16];
            for (bits, slot) in iron_bars.iter_mut().enumerate() {
                let sides = ["north", "south", "west", "east"];
                *slot = missing(with_bits(bars, &sides, bits), &bars.identifier)?;
            }
            Generator::EndSpikes(Box::new(CompiledEndSpikes {
                spikes: if spikes.is_empty() {
                    seed_spikes(resolver.world_seed)
                } else {
                    spikes.clone()
                },
                obsidian: resolver.default_state("minecraft:obsidian")?,
                bedrock: resolver.default_state("minecraft:bedrock")?,
                fire: resolver.default_state("minecraft:fire")?,
                iron_bars,
            }))
        }
        Feature::Speleothem {
            base_block,
            pointed_block,
            replaceable_blocks,
            chance_of_taller_generation,
            chance_of_directional_spread,
            chance_of_spread_radius2,
            chance_of_spread_radius3,
        } => {
            let base_block_states = resolver.block_mask(base_block.name.as_str())?;
            let replaceable = resolver.mask(StateQuery::Blocks(replaceable_blocks))?;
            Generator::Speleothem(Box::new(CompiledSpeleothem {
                base_block: resolver.resolve(base_block)?,
                pointed: missing(
                    PointedStates::resolve(pointed_block, resolver),
                    state_named(pointed_block),
                )?,
                base_or_replaceable: union_masks(&[&base_block_states, &replaceable]),
                replaceable_blocks: replaceable,
                chance_of_taller_generation: chance_of_taller_generation.0 as f32,
                chance_of_directional_spread: chance_of_directional_spread.0 as f32,
                chance_of_spread_radius2: chance_of_spread_radius2.0 as f32,
                chance_of_spread_radius3: chance_of_spread_radius3.0 as f32,
            }))
        }
        Feature::CoralTree { feature } => {
            Generator::CoralTree(Box::new(compile_nested(feature, trees, resolver, corpus)?))
        }
        Feature::CoralClaw { feature } => {
            Generator::CoralClaw(Box::new(compile_nested(feature, trees, resolver, corpus)?))
        }
        Feature::SingleBlockPillar {
            block,
            can_replace,
            direction,
            chance_to_continue,
            cap_feature,
        } => Generator::SingleBlockPillar {
            config: Box::new(CompiledSingleBlockPillar {
                block: compile_provider(block, resolver)?,
                can_replace: can_replace
                    .as_ref()
                    .map_or(Ok(Predicate::True), |p| compile_predicate(p, resolver))?,
                direction: (*direction).into(),
                chance_to_continue: chance_to_continue.0 as f32,
            }),
            cap: cap_feature
                .as_ref()
                .map(|holder| compile_nested(holder, trees, resolver, corpus).map(Box::new))
                .transpose()?,
        },
        Feature::ProjectedRandomPatchySquare {
            block,
            project_through,
            size,
            max_projection_height,
        } => Generator::ProjectedPatchySquare(Box::new(CompiledProjectedPatchySquare {
            block: compile_provider(block, resolver)?,
            project_through: compile_predicate(project_through, resolver)?,
            size: size.0.clone(),
            max_projection_height: max_projection_height.0,
        })),
        Feature::SteppedColumnCluster {
            block,
            continue_through,
            can_replace,
            cannot_place_on,
            cluster_reach,
            column_count,
            column_reach,
            height,
        } => Generator::SteppedColumnCluster(Box::new(CompiledSteppedColumnCluster {
            block: compile_provider(block, resolver)?,
            continue_through: compile_predicate(continue_through, resolver)?,
            can_replace: compile_predicate(can_replace, resolver)?,
            cannot_place_on: resolver.mask(StateQuery::Blocks(cannot_place_on))?,
            cluster_reach: cluster_reach.clone(),
            column_count: column_count.clone(),
            column_reach: column_reach.clone(),
            height: height.clone(),
        })),
        Feature::SculkPatch {
            charge_count,
            amount_per_charge,
            spread_attempts,
            growth_rounds,
            spread_rounds,
        } => Generator::SculkPatch(Box::new(CompiledSculkPatch {
            charge_count: charge_count.0,
            amount_per_charge: amount_per_charge.0,
            spread_attempts: spread_attempts.0,
            growth_rounds: growth_rounds.0,
            spread_rounds: spread_rounds.0,
            vein: missing(
                multiface_states(resolver.blocks, "minecraft:sculk_vein"),
                "minecraft:sculk_vein",
            )?,
            sculk: resolver.default_state("minecraft:sculk")?,
            sculk_states: resolver.block_mask("minecraft:sculk")?,
            blocks_vein: resolver.blocks_mask(&[
                "minecraft:sculk",
                "minecraft:sculk_catalyst",
                "minecraft:moving_piston",
            ])?,
            fire: resolver.tag_mask("minecraft:fire")?,
            replaceable_world_gen: resolver.tag_mask("minecraft:sculk_replaceable_world_gen")?,
            substrate: resolver.tag_mask("minecraft:sculk_replaceable")?,
            growth_inhibitors: resolver.tag_mask("minecraft:sculk_growth_inhibitors")?,
            sensor: waterlogged_pair(resolver, "minecraft:sculk_sensor", &[])?,
            shrieker: waterlogged_pair(
                resolver,
                "minecraft:sculk_shrieker",
                &[("can_summon", "true")],
            )?,
        })),
        Feature::NoOp => Generator::NoOp,
        Feature::BetaPopulate => Generator::BetaPopulate(Box::new(BetaPopulate {
            ids: BetaOreBlockIds::resolve(resolver.blocks),
            world_seed: resolver.world_seed,
        })),
        Feature::FillLayer { height, state } => Generator::FillLayer(CompiledFillLayer {
            height: height.0,
            state: resolver.resolve(state)?,
        }),
        Feature::ReplaceSingleBlock { targets } => {
            Generator::ReplaceSingleBlock(CompiledReplaceSingleBlock {
                targets: targets
                    .iter()
                    .map(|entry| {
                        Ok(Replacement {
                            target: compile_rule(&entry.target, resolver)?,
                            state: resolver.resolve(&entry.state)?,
                        })
                    })
                    .collect::<Compiled<_>>()?,
            })
        }
        Feature::Fossil {
            fossil_structures,
            overlay_structures,
            fossil_processors,
            overlay_processors,
            max_empty_corners_allowed,
        } => {
            if fossil_structures.len() != overlay_structures.len() {
                return Err(FeatureCompileError::Template(
                    "fossil structure lists must be equal lengths".to_owned(),
                ));
            }
            let pairs = fossil_structures
                .iter()
                .zip(overlay_structures)
                .map(|(fossil, overlay)| {
                    Ok((
                        freeze_feature_template(corpus, resolver, fossil)?,
                        freeze_feature_template(corpus, resolver, overlay)?,
                    ))
                })
                .collect::<Compiled<Vec<_>>>()?;
            let chain = |processors| {
                compile_chain(
                    feature_processors(corpus, Some(processors))?,
                    ChainKind::Feature,
                    resolver,
                    resolver.world_seed,
                )
            };
            Generator::Fossil(Box::new(CompiledFossil {
                pairs,
                fossil_chain: chain(fossil_processors)?,
                overlay_chain: chain(overlay_processors)?,
                max_empty_corners: max_empty_corners_allowed.0 as usize,
            }))
        }
        Feature::Template {
            templates,
            processors,
        } => {
            let entries = templates
                .iter()
                .map(|entry| {
                    let frozen = freeze_feature_template(corpus, resolver, &entry.data.id)?;
                    let rotations = entry
                        .data
                        .rotations
                        .clone()
                        .unwrap_or_else(|| Rotation::ALL.to_vec());
                    Ok((entry.weight.0, frozen, rotations))
                })
                .collect::<Compiled<Vec<_>>>()?;
            let chain = compile_chain(
                feature_processors(corpus, processors.as_ref())?,
                ChainKind::Feature,
                resolver,
                resolver.world_seed,
            )?;
            Generator::Template(Box::new(CompiledTemplateFeature { entries, chain }))
        }
    })
}

fn freeze_feature_template(
    corpus: &LoadedFeatures,
    resolver: &Resolver<'_>,
    id: &ResourceLocation,
) -> Compiled<FrozenTemplate> {
    let template = corpus
        .templates
        .get(id)
        .ok_or_else(|| FeatureCompileError::UnknownTemplate(id.clone()))?;
    let (frozen, _) = template
        .freeze(id, &|state| resolve_palette_state(resolver.blocks, state))
        .map_err(|error| FeatureCompileError::Template(error.to_string()))?;
    check_block_entity_ids(id, &frozen).map_err(FeatureCompileError::Template)?;
    Ok(frozen)
}

fn feature_processors<'a>(
    corpus: &'a LoadedFeatures,
    processors: Option<&'a Holder<StructureProcessorList>>,
) -> Compiled<&'a [StructureProcessor]> {
    Ok(match processors {
        Some(Holder::Reference(id)) => corpus
            .processor_lists
            .get(id)
            .map(processor_list)
            .ok_or_else(|| FeatureCompileError::UnknownProcessorList(id.clone()))?,
        Some(Holder::Inline(list)) => processor_list(list),
        None => &[],
    })
}

/// `FluidState.createLegacyBlock` over the shapes a spring's `state` takes.
///
/// The field is a fluid state, not a block state: `falling` is a property of
/// the fluid and never reaches the block, and a source fluid's legacy block is
/// the fluid block's own default. A flowing fluid would need a `level` the
/// block registry cannot be asked for through this shape, so it is a load
/// error rather than a silently wrong state.
fn spring_state(state: &BlockState, blocks: &dyn BlockResolver) -> Option<VoxelId> {
    if state
        .properties
        .iter()
        .flatten()
        .any(|(property, _)| property == "level")
    {
        return None;
    }
    blocks.state(&BlockState {
        name: state.name.clone(),
        properties: None,
    })
}

fn unsupported(what: &str) -> FeatureCompileError {
    FeatureCompileError::Unsupported(what.to_owned())
}

/// `None` names a block or state the corpus does not hold.
pub(super) fn missing<T>(value: Option<T>, what: impl std::fmt::Display) -> Compiled<T> {
    value.ok_or_else(|| FeatureCompileError::UnknownBlockState(what.to_string()))
}

/// One block's state under the given properties, dry and waterlogged.
fn waterlogged_pair(
    resolver: &Resolver<'_>,
    block: &str,
    properties: &[(&str, &str)],
) -> Compiled<[VoxelId; 2]> {
    let dry = missing(state_of(resolver.blocks, block, properties), block)?;
    let wet = set(
        resolver.blocks.owner(BlockStateId(dry.0)),
        BlockStateId(dry.0),
        "waterlogged",
        "true",
    )?;
    Ok([dry, VoxelId::from(wet.0)])
}

/// `BlockState.setValue` at freeze, naming the state it could not reach.
fn set(
    entry: &BlockEntry,
    id: BlockStateId,
    property: &str,
    value: &str,
) -> Compiled<BlockStateId> {
    missing(
        entry.with_text(id, property, value),
        format_args!("{}[{property}={value}]", entry.identifier),
    )
}

/// The default state with each of `properties` set from one bit of `bits`,
/// the first property the highest bit.
fn with_bits(entry: &BlockEntry, properties: &[&str], bits: usize) -> Option<VoxelId> {
    with_bits_from(entry, entry.default_state_id, properties, bits)
}

fn with_bits_from(
    entry: &BlockEntry,
    base: BlockStateId,
    properties: &[&str],
    bits: usize,
) -> Option<VoxelId> {
    let id = properties
        .iter()
        .enumerate()
        .try_fold(base, |id, (index, property)| {
            let on = bits >> (properties.len() - 1 - index) & 1 == 1;
            entry.with_text(id, property, if on { "true" } else { "false" })
        })?;
    Some(VoxelId::from(id.0))
}

const DELTA_CANNOT_REPLACE: &[&str] = &[
    "minecraft:bedrock",
    "minecraft:nether_bricks",
    "minecraft:nether_brick_fence",
    "minecraft:nether_brick_stairs",
    "minecraft:nether_wart",
    "minecraft:chest",
    "minecraft:spawner",
];

fn location(id: &str) -> ResourceLocation {
    ResourceLocation::parse(id).expect("a literal id")
}

pub(super) fn union_masks(masks: &[&StateMask]) -> StateMask {
    let mut out = FixedBitSet::new();
    for mask in masks {
        out.union_with(mask);
    }
    Arc::new(out)
}

/// The two halves of every double plant, keyed by the lower one the provider
/// hands back. A block whose `half` runs `lower`/`upper` is one; `top`/`bottom`
/// is a stair or a slab and never reaches here.
/// Every state of `pale_moss_carpet` both ways, and the four masks a side
/// asks of the block it would climb. `None` where the corpus has no such
/// block, which is what a datapack that drops it leaves behind.
fn mossy_carpet_states(resolver: &Resolver<'_>) -> Compiled<Option<MossyCarpetStates>> {
    const BLOCK: &str = "minecraft:pale_moss_carpet";
    const SIDES: [WallSide; 3] = [WallSide::None, WallSide::Low, WallSide::Tall];
    const FACES: [&str; 4] = ["north", "east", "south", "west"];

    if resolver.blocks.block(BLOCK).is_none() {
        return Ok(None);
    }

    let text = |side: WallSide| match side {
        WallSide::None => "none",
        WallSide::Low => "low",
        WallSide::Tall => "tall",
    };

    let mut by_shape = [VoxelId::default(); SHAPE_COUNT];
    let mut by_state = FxHashMap::default();
    for base in [false, true] {
        for north in SIDES {
            for east in SIDES {
                for south in SIDES {
                    for west in SIDES {
                        let shape = CarpetShape {
                            base,
                            sides: [north, east, south, west],
                        };
                        let mut properties = vec![("bottom", if base { "true" } else { "false" })];
                        properties.extend(
                            FACES
                                .iter()
                                .zip(shape.sides)
                                .map(|(face, side)| (*face, text(side))),
                        );
                        let state = missing(state_of(resolver.blocks, BLOCK, &properties), BLOCK)?;
                        by_shape[shape.index()] = state;
                        by_state.insert(state.0, shape);
                    }
                }
            }
        }
    }

    // The carpet climbs the neighbour's face that points back at it.
    let mut attaches = Vec::with_capacity(4);
    for direction in Direction::HORIZONTAL {
        attaches.push(resolver.mask(StateQuery::SturdyFace(direction.opposite()))?);
    }

    Ok(Some(MossyCarpetStates {
        by_shape,
        by_state,
        attaches: attaches.try_into().expect("four horizontals"),
        air: resolver.world.air,
        world_seed: resolver.world_seed,
    }))
}

fn double_plant_table(blocks: &BlockDefinitions) -> FxHashMap<u16, DoublePlant> {
    let mut table = FxHashMap::default();
    for (state, value) in with_property(blocks, "half") {
        if !value.renders_to("lower") {
            continue;
        }
        let id = BlockStateId(state.0);
        let entry = blocks.owner(id);
        let Some(upper) = entry.with_text(id, "half", "upper") else {
            continue;
        };
        let waterlogged = entry
            .with_text(id, "waterlogged", "true")
            .zip(entry.with_text(upper, "waterlogged", "true"))
            .map(|(lower, upper)| (VoxelId::from(lower.0), VoxelId::from(upper.0)));
        table.insert(
            id.0,
            DoublePlant {
                lower: VoxelId::from(id.0),
                upper: VoxelId::from(upper.0),
                waterlogged,
            },
        );
    }
    table
}

/// Every state carrying `snowy`, mapped to the same state with it set.
fn snowy_table(blocks: &BlockDefinitions) -> FxHashMap<VoxelId, VoxelId> {
    with_property(blocks, "snowy")
        .filter_map(|(state, _)| {
            let id = BlockStateId(state.0);
            let snowy = blocks.owner(id).with_text(id, "snowy", "true")?;
            Some((state, VoxelId::from(snowy.0)))
        })
        .collect()
}

fn waterlogging(blocks: &BlockDefinitions, water: VoxelId) -> Waterlogging {
    let flooded = with_property(blocks, "waterlogged")
        .filter(|(_, value)| !value.renders_to("true"))
        .filter_map(|(state, _)| {
            let id = BlockStateId(state.0);
            let flooded = blocks.owner(id).with_text(id, "waterlogged", "true")?;
            Some((state.0, VoxelId::from(flooded.0)))
        })
        .collect();
    Waterlogging { water, flooded }
}

/// One block's states in `Direction::HORIZONTAL` order, by its `facing` property.
fn horizontal_facings(blocks: &BlockDefinitions, block: &str) -> Option<[VoxelId; 4]> {
    let entry = blocks.block(block)?;
    let mut states = [VoxelId(0); 4];
    for (index, direction) in Direction::HORIZONTAL.into_iter().enumerate() {
        let id = entry.with_text(entry.default_state_id, "facing", direction.name())?;
        states[index] = VoxelId::from(id.0);
    }
    Some(states)
}

/// `minecraft:vine` attached on each single face, in `Direction::all()` order.
fn vine_states(blocks: &BlockDefinitions) -> Option<[VoxelId; 6]> {
    let default = state_of(blocks, "minecraft:vine", &[])?;
    let mut states = [default; 6];
    for (index, direction) in Direction::all().into_iter().enumerate() {
        if direction != Direction::Down {
            states[index] = state_of(blocks, "minecraft:vine", &[(direction.name(), "true")])?;
        }
    }
    Some(states)
}

fn multiface_states(blocks: &BlockDefinitions, block: &str) -> Option<MultifaceStates> {
    let entry = blocks.block(block)?;
    let mut by_faces = [[VoxelId(0); 64]; 2];
    // The first face is the lowest bit here, so the property list is reversed.
    let mut faces_high_first = Direction::all().map(|direction| direction.name());
    faces_high_first.reverse();
    for (waterlogged, row) in by_faces.iter_mut().enumerate() {
        for faces in 0..64usize {
            let id = with_bits(entry, &faces_high_first, faces)?;
            let id = entry
                .with_text(
                    BlockStateId(id.0),
                    "waterlogged",
                    if waterlogged == 1 { "true" } else { "false" },
                )
                .map_or(id, |flooded| VoxelId::from(flooded.0));
            row[faces] = id;
        }
    }
    let mut of_state = FxHashMap::default();
    for offset in 0..entry.state_count {
        let id = BlockStateId(entry.base_state_id.0 + offset);
        let mut faces = 0u8;
        for (bit, direction) in Direction::all().into_iter().enumerate() {
            if entry
                .value_of(id, direction.name())
                .is_some_and(|value| value.renders_to("true"))
            {
                faces |= 1 << bit;
            }
        }
        let waterlogged = entry
            .value_of(id, "waterlogged")
            .is_some_and(|value| value.renders_to("true"));
        of_state.insert(id.0, (faces, waterlogged));
    }
    Some(MultifaceStates { by_faces, of_state })
}

/// Every state of every block declaring the five `HugeMushroomBlock` faces,
/// with the state reached by setting each combination of them.
fn mushroom_faces(blocks: &BlockDefinitions) -> MushroomFaces {
    const FACES: [&str; 5] = ["up", "west", "east", "north", "south"];
    let mut table = MushroomFaces::default();
    for entry in blocks.blocks() {
        if FACES
            .iter()
            .any(|face| entry.properties.index_of(face).is_none())
        {
            continue;
        }
        for offset in 0..entry.state_count {
            let id = BlockStateId(entry.base_state_id.0 + offset);
            let up = entry
                .value_of(id, "up")
                .is_some_and(|value| value.renders_to("true"));
            let mut by_faces = [VoxelId(0); 32];
            for (index, slot) in by_faces.iter_mut().enumerate() {
                *slot = with_bits_from(entry, id, &FACES, index).unwrap_or(VoxelId::from(id.0));
            }
            table.insert(VoxelId::from(id.0), up, by_faces);
        }
    }
    table
}

fn compile_vegetation_patch(
    config: &mcrs_minecraft_worldgen_feature::proto::VegetationPatchConfig,
    waterlogged: bool,
    trees: &Arc<TreeTables>,
    resolver: &Resolver<'_>,
    corpus: &LoadedFeatures,
) -> Compiled<Generator> {
    let compiled = CompiledVegetationPatch {
        replaceable: resolver.mask(StateQuery::Blocks(&config.replaceable))?,
        ground_state: compile_provider(&config.ground_state, resolver)?,
        surface: config.surface,
        depth: config.depth.0.clone(),
        extra_bottom_block_chance: config.extra_bottom_block_chance.0 as f32,
        vertical_range: config.vertical_range.0,
        vegetation_chance: config.vegetation_chance.0 as f32,
        xz_radius: config.xz_radius.clone(),
        extra_edge_column_chance: config.extra_edge_column_chance.0 as f32,
        waterlogged,
        tables: resolver.tables.clone(),
    };
    Ok(Generator::VegetationPatch {
        config: Box::new(compiled),
        feature: Box::new(compile_nested(
            &config.vegetation_feature,
            trees,
            resolver,
            corpus,
        )?),
    })
}

fn compile_nested(
    holder: &Holder<PlacedFeature>,
    trees: &Arc<TreeTables>,
    resolver: &Resolver<'_>,
    corpus: &LoadedFeatures,
) -> Compiled<Nested> {
    let placed = match holder {
        Holder::Inline(placed) => placed.as_ref(),
        Holder::Reference(id) => corpus
            .placed_features
            .get(id)
            .ok_or_else(|| FeatureCompileError::UnknownPlacedFeature(id.clone()))?,
    };
    compile_placed(placed, trees, resolver, corpus)
}

/// One placed feature, top-level or nested: its chain, and its generator when
/// this build has code for the shape. A nested feature without one keeps its
/// chain and its draws, so an unsupported leaf is not an error here; the
/// top level decides what a missing generator means.
fn compile_placed(
    placed: &PlacedFeature,
    trees: &Arc<TreeTables>,
    resolver: &Resolver<'_>,
    corpus: &LoadedFeatures,
) -> Compiled<Nested> {
    let feature = match &placed.feature {
        Holder::Inline(feature) => feature.as_ref(),
        Holder::Reference(id) => corpus
            .features
            .get(id)
            .ok_or_else(|| FeatureCompileError::UnknownFeature(id.clone()))?,
    };
    let placement = compile_placement(&placed.placement, resolver)?;
    unsupported_survive_state(&placement, trees, resolver)?;
    let generator = match compile_generator(feature, trees, resolver, corpus) {
        Ok(generator) => Some(generator),
        Err(error) if error.is_unsupported() => {
            tracing::warn!(%error, "a nested feature places nothing");
            None
        }
        Err(error) => return Err(error),
    };
    Ok(Nested {
        placement,
        generator,
    })
}

/// A `HolderSet` of placed features as a list. No `tags/worldgen/placed_feature`
/// registry reaches here, so a tag cannot be expanded.
fn holders(set: &PlacedFeatureSet) -> Compiled<&[Holder<PlacedFeature>]> {
    match set {
        HolderSet::Tag(tag) => Err(FeatureCompileError::UnknownTag(tag.clone())),
        _ => Ok(set.entries()),
    }
}

/// A `would_survive` in the chain naming a state no family answers.
fn unsupported_survive_state(
    placement: &[Modifier],
    trees: &TreeTables,
    resolver: &Resolver<'_>,
) -> Compiled<()> {
    fn walk(predicate: &Predicate, trees: &TreeTables, resolver: &Resolver<'_>) -> Option<String> {
        match predicate {
            Predicate::WouldSurvive { state, .. } => resolver.without_survive_rule(trees, *state),
            Predicate::VolumeMatch { matches, .. } => walk(matches, trees, resolver),
            Predicate::Not(inner) => walk(inner, trees, resolver),
            Predicate::AnyOf(inner) | Predicate::AllOf(inner) => {
                inner.iter().find_map(|p| walk(p, trees, resolver))
            }
            _ => None,
        }
    }
    let unanswered = placement.iter().find_map(|modifier| match modifier {
        Modifier::BlockPredicateFilter { predicate } => walk(predicate, trees, resolver),
        _ => None,
    });
    match unanswered {
        Some(state) => Err(unsupported(&format!("would_survive {state}"))),
        None => Ok(()),
    }
}

fn compile_ore(
    targets: &[BlockReplacement],
    size: i32,
    discard_chance_on_air_exposure: f32,
    resolver: &Resolver<'_>,
) -> Compiled<CompiledOre> {
    let mut replacements = Vec::with_capacity(targets.len());
    for target in targets {
        let state = resolver.resolve(&target.state)?;
        replacements.push(OreReplacement {
            target: compile_rule(&target.target, resolver)?,
            state,
        });
    }
    Ok(CompiledOre {
        targets: replacements,
        size,
        discard_chance_on_air_exposure,
    })
}

/// A block's properties as the digits of its state ids, the way
/// `BlockEntry::with` computes them.
fn block_layout(entry: &BlockEntry) -> BlockLayout {
    let properties = &entry.properties.0;
    BlockLayout {
        base: entry.base_state_id.0,
        properties: properties
            .iter()
            .enumerate()
            .map(|(index, property)| {
                let values: Arc<[Arc<str>]> = property
                    .values
                    .iter()
                    .map(|value| match value {
                        PropertyValue::Str(text) => Arc::from(&**text),
                        PropertyValue::Int(int) => Arc::from(int.to_string()),
                        PropertyValue::Bool(flag) => {
                            Arc::from(if *flag { "true" } else { "false" })
                        }
                    })
                    .collect();
                let stride = properties[index + 1..]
                    .iter()
                    .map(|p| p.values.len() as u16)
                    .product();
                PropertyLayout {
                    name: Arc::from(&*property.name),
                    values,
                    stride,
                }
            })
            .collect(),
    }
}

pub struct Resolver<'a> {
    pub blocks: &'a BlockDefinitions,
    pub world: WorldStates,
    /// The state tables no feature configures, beside the world-wide state sets
    /// and resolved with them: one per dimension, shared by every feature that
    /// reads one.
    pub tables: Arc<BlockTables>,
    pub tags: Option<&'a DynTagRegistry<VanillaBlock>>,
    pub fluid_tags: Option<&'a DynTagRegistry<Fluid>>,
    pub biomes: &'a RegistrySnapshot<Biome>,
    /// The geode's noise and the End's spike ring are drawn from the world seed
    /// rather than from the object's own source, so they belong to the freeze.
    pub world_seed: i64,
    /// Indexed by the biome id [`WorldGenVolume::biome`] answers with.
    pub climate: &'a [BiomeClimate],
}

impl<'a> Resolver<'a> {
    pub fn new(
        blocks: &'a BlockDefinitions,
        tags: Option<&'a DynTagRegistry<VanillaBlock>>,
        fluid_tags: Option<&'a DynTagRegistry<Fluid>>,
        biomes: &'a RegistrySnapshot<Biome>,
        world_seed: i64,
        climate: &'a [BiomeClimate],
    ) -> Compiled<Self> {
        let mut resolver = Resolver {
            blocks,
            world: WorldStates::default(),
            tables: Arc::new(BlockTables::default()),
            tags,
            fluid_tags,
            biomes,
            world_seed,
            climate,
        };
        resolver.world = resolver.world_states();
        resolver.tables = Arc::new(resolver.block_tables()?);
        Ok(resolver)
    }

    /// Every table that is a function of the block definitions alone. Built
    /// here so the corpus's seventy-five `simple_block` entries share one copy
    /// rather than each compiling its own.
    fn block_tables(&self) -> Compiled<BlockTables> {
        let water = self.blocks.default_state("minecraft:water");
        Ok(BlockTables {
            double_plants: double_plant_table(self.blocks),
            mossy_carpet: mossy_carpet_states(self)?,
            snowy: snowy_table(self.blocks),
            waterlogging: waterlogging(self.blocks, VoxelId::from(water.0)),
            vine: vine_states(self.blocks),
            mushroom_faces: mushroom_faces(self.blocks),
        })
    }

    /// The world-wide state sets, resolved once for every feature to share.
    fn world_states(&self) -> WorldStates {
        let water = self.states(StateQuery::Fluids(&HolderSet::One(location(
            "minecraft:water",
        ))));
        let lava = self.states(StateQuery::Fluids(&HolderSet::One(location(
            "minecraft:lava",
        ))));
        let block = |name: &str| self.block_mask(name).unwrap_or_default();
        WorldStates {
            air: self.default_state("minecraft:air").unwrap_or_default(),
            cave_air: self.default_state("minecraft:cave_air").unwrap_or_default(),
            water: self.default_state("minecraft:water").unwrap_or_default(),
            lava: self.default_state("minecraft:lava").unwrap_or_default(),
            air_states: self.flag_mask(BlockStateFlags::IS_AIR),
            water_states: block("minecraft:water"),
            lava_states: block("minecraft:lava"),
            water_fluid: water.clone().unwrap_or_default(),
            water_source: self.state_mask(|state| {
                state.fluid.is_some_and(|f| {
                    f.source && self.blocks.fluid(f.fluid).as_str() == "minecraft:water"
                })
            }),
            lava_fluid: lava.clone().unwrap_or_default(),
            any_source_fluid: self.state_mask(|state| state.fluid.is_some_and(|f| f.source)),
            any_fluid: union_masks(&[&water.unwrap_or_default(), &lava.unwrap_or_default()]),
            replaceable: self.flag_mask(BlockStateFlags::REPLACEABLE),
            solid: self.flag_mask(BlockStateFlags::LEGACY_SOLID),
            solid_render: self.flag_mask(BlockStateFlags::IS_SOLID_RENDER),
            sturdy_up: self.flag_mask(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK),
            center_down: Arc::new(crate::trees::face_support(self.blocks).center_down),
            empty_collision: self
                .state_mask(|state| self.blocks.shape(state.collision_shape).is_empty()),
            bedrock: block("minecraft:bedrock"),
            unrotated: union_masks(&[&block("minecraft:fire"), &block("minecraft:chorus_plant")]),
            unmirrored: union_masks(&[
                &block("minecraft:fire"),
                &block("minecraft:chorus_plant"),
                &block("minecraft:anvil"),
                &block("minecraft:chipped_anvil"),
                &block("minecraft:damaged_anvil"),
            ]),
            has_block_entity: self.flag_mask(BlockStateFlags::HAS_BLOCK_ENTITY),
            block_of_state: (0..self.blocks.state_count())
                .map(|id| self.blocks.block_index(BlockStateId(id as u16)))
                .collect(),
            layouts: self.blocks.blocks().iter().map(block_layout).collect(),
        }
    }

    pub fn block(&self, id: &str) -> Compiled<&BlockEntry> {
        missing(self.blocks.block(id), id)
    }

    pub fn resolve(&self, state: &BlockState) -> Compiled<VoxelId> {
        resolve_state(self, state)
    }

    pub fn mask(&self, query: StateQuery<'_>) -> Compiled<StateMask> {
        states_of(self, query)
    }

    pub fn tag_mask(&self, tag: &str) -> Compiled<StateMask> {
        self.mask(StateQuery::BlockTag(&location(tag)))
    }

    pub fn blocks_mask(&self, names: &[&str]) -> Compiled<StateMask> {
        let ids = names.iter().map(|name| location(name)).collect();
        self.mask(StateQuery::Blocks(&HolderSet::List(ids)))
    }

    pub fn default_state(&self, block: &str) -> Compiled<VoxelId> {
        Ok(VoxelId::from(self.block(block)?.default_state_id.0))
    }

    /// Every state `keep` answers for.
    pub fn state_mask(&self, keep: impl Fn(&BlockStateData) -> bool) -> StateMask {
        let mut mask: FixedBitSet = (0..self.blocks.state_count())
            .filter(|&id| keep(self.blocks.state(BlockStateId(id as u16))))
            .collect();
        mask.grow(self.blocks.state_count());
        Arc::new(mask)
    }

    /// The biomes cold enough for the lake's ice pass, which is the reference's
    /// `coldEnoughToSnow` reduced to the base temperature.
    pub fn freezing_biomes(&self) -> BiomeMask {
        let mut mask: FixedBitSet = (0..self.climate.len())
            .filter(|&slot| self.climate[slot].base_temperature < 0.15)
            .collect();
        mask.grow(self.climate.len());
        Arc::new(mask)
    }

    pub fn flag_mask(&self, flag: BlockStateFlags) -> StateMask {
        self.state_mask(|state| state.flags.contains(flag))
    }

    pub fn block_mask(&self, block: &str) -> Compiled<StateMask> {
        self.mask(StateQuery::Block(&location(block)))
    }

    /// The default state of every block of a set, in the set's own order — what
    /// `random_block_provider` draws one of.
    pub fn block_set_defaults(&self, set: &HolderSet) -> Compiled<Vec<VoxelId>> {
        match set {
            HolderSet::Tag(tag) => self
                .tag_entries(tag)
                .ok_or_else(|| FeatureCompileError::UnknownBlockSet(format!("#{tag}")))?
                .map(|entry| {
                    missing(entry, tag).map(|entry| VoxelId::from(entry.default_state_id.0))
                })
                .collect(),
            HolderSet::One(id) => Ok(vec![self.default_state(id.as_str())?]),
            HolderSet::List(ids) => ids
                .iter()
                .map(|id| self.default_state(id.as_str()))
                .collect(),
        }
    }

    /// The block's name when no `canSurvive` rule covers it, which makes a
    /// `would_survive` naming it unanswerable.
    pub fn without_survive_rule(&self, trees: &TreeTables, state: VoxelId) -> Option<String> {
        let block = self.blocks.block_index(BlockStateId(state.0));
        if trees.survive.contains_key(&block) {
            return None;
        }
        let entry = self.blocks.blocks().get(block as usize)?;
        Some(entry.identifier.as_str().to_owned())
    }

    fn add_entry(mask: &mut FixedBitSet, entry: &BlockEntry) {
        let base = entry.base_state_id.0 as usize;
        mask.insert_range(base..base + entry.state_count as usize);
    }

    fn add_block(&self, mask: &mut FixedBitSet, id: &ResourceLocation) -> Option<()> {
        Some(Self::add_entry(mask, self.blocks.block(id.as_str())?))
    }

    /// The blocks a tag names, each `None` where the tag points past the registry.
    fn tag_entries(
        &self,
        tag: &ResourceLocation,
    ) -> Option<impl Iterator<Item = Option<&BlockEntry>>> {
        let key: TagKey<VanillaBlock, Arc<str>> = TagKey::from_location(tag.clone());
        let members = self.tags?.get(&key)?;
        Some(
            members
                .iter()
                .map(|index| self.blocks.blocks().get(index as usize)),
        )
    }

    fn add_tag(&self, mask: &mut FixedBitSet, tag: &ResourceLocation) -> Option<()> {
        self.tag_entries(tag)?
            .try_for_each(|entry| Some(Self::add_entry(mask, entry?)))
    }

    fn add_set(&self, mask: &mut FixedBitSet, set: &HolderSet) -> Option<()> {
        match set {
            HolderSet::Tag(tag) => self.add_tag(mask, tag),
            HolderSet::One(id) => self.add_block(mask, id),
            HolderSet::List(ids) => ids.iter().try_for_each(|id| self.add_block(mask, id)),
        }
    }
}

impl BlockResolver for Resolver<'_> {
    fn state(&self, state: &BlockState) -> Option<VoxelId> {
        crate::block_state::try_resolve_state(self.blocks, state).map(|id| VoxelId::from(id.0))
    }

    fn states(&self, query: StateQuery<'_>) -> Option<StateMask> {
        let mut mask = FixedBitSet::with_capacity(self.blocks.state_count());
        match query {
            StateQuery::Blocks(set) => self.add_set(&mut mask, set)?,
            StateQuery::Block(id) => self.add_block(&mut mask, id)?,
            StateQuery::BlockTag(tag) => self.add_tag(&mut mask, tag)?,
            StateQuery::Fluids(set) => {
                let names: Vec<&str> = match set {
                    HolderSet::Tag(tag) => {
                        let key: TagKey<Fluid, Arc<str>> = TagKey::from_location(tag.clone());
                        self.fluid_tags?
                            .get(&key)?
                            .iter()
                            .map(|index| self.blocks.fluid(FluidId(index as u16)).as_str())
                            .collect()
                    }
                    HolderSet::One(id) => vec![id.as_str()],
                    HolderSet::List(ids) => ids.iter().map(|id| id.as_str()).collect(),
                };
                // No block definition interns `minecraft:empty`, but the
                // reference's fluid-less states all carry it, so it matches
                // everything that holds no fluid rather than nothing at all.
                let empty = names.contains(&"minecraft:empty");
                return Some(self.state_mask(|state| match state.fluid {
                    None => empty,
                    Some(fluid) => names.contains(&self.blocks.fluid(fluid.fluid).as_str()),
                }));
            }
            StateQuery::Solid => return Some(self.world.solid.clone()),
            StateQuery::Replaceable => return Some(self.world.replaceable.clone()),
            StateQuery::FullOutline => {
                let mut full: FxHashMap<u32, bool> = FxHashMap::default();
                for id in 0..self.blocks.state_count() {
                    let shape = self.blocks.state(BlockStateId(id as u16)).selection_shape;
                    if *full.entry(shape.0).or_insert_with(|| {
                        VoxelShape::from_boxes(self.blocks.shape(shape)).occludes_full_block()
                    }) {
                        mask.insert(id);
                    }
                }
            }
            StateQuery::SturdyFace(direction) => {
                let face = mcrs_minecraft_core::Direction::all()[direction as usize];
                let mut covers: FxHashMap<u32, bool> = FxHashMap::default();
                let mut full = |shape: mcrs_minecraft_block::definition::ShapeId| {
                    *covers.entry(shape.0).or_insert_with(|| {
                        *VoxelShape::from_boxes(self.blocks.shape(shape)).face_mask(face)
                            == FACE_MASK_FULL
                    })
                };
                for id in 0..self.blocks.state_count() {
                    let state = self.blocks.state(BlockStateId(id as u16));
                    if full(state.selection_shape) || full(state.collision_shape) {
                        mask.insert(id);
                    }
                }
            }
            StateQuery::FullCollisionFace(direction) => {
                let face = mcrs_minecraft_core::Direction::all()[direction as usize];
                let mut covers: FxHashMap<u32, bool> = FxHashMap::default();
                for id in 0..self.blocks.state_count() {
                    let shape = self.blocks.state(BlockStateId(id as u16)).collision_shape;
                    if *covers.entry(shape.0).or_insert_with(|| {
                        *VoxelShape::from_boxes(self.blocks.shape(shape)).face_mask(face)
                            == FACE_MASK_FULL
                    }) {
                        mask.insert(id);
                    }
                }
            }
        }
        Some(Arc::new(mask))
    }

    fn biomes(&self, set: &HolderSet) -> Option<BiomeMask> {
        let mut mask = FixedBitSet::with_capacity(self.biomes.len() as usize);
        match set {
            HolderSet::Tag(_) => return None,
            HolderSet::One(id) => mask.insert(self.biomes.by_location(id.as_str())? as usize),
            HolderSet::List(ids) => {
                for id in ids {
                    mask.insert(self.biomes.by_location(id.as_str())? as usize);
                }
            }
        }
        Some(Arc::new(mask))
    }
}
