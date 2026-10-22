use crate::beard::{BeardifierPlacement, beardifier_placement};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{IdSet, KeySet, Registries, RegistryOf, Resolve};
use mcrs_minecraft_worldgen_density::compile::CompileError;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter, RouterBlocks};
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_surface::compile::{MaterialProgram, build_router_and_material};
use mcrs_minecraft_worldgen_surface::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
};
use std::collections::BTreeMap;

// chisle: the compiler takes its registries keyed by name, so every entry of
// each is cloned once per dimension at start. A compiler that takes ids and
// reads the registries lifts it.
fn named<T: Clone + Send + Sync + 'static>(
    registries: &Registries,
) -> BTreeMap<ResourceLocation, T> {
    registries
        .registry::<RegistryOf<T>>()
        .into_iter()
        .flat_map(|registry| registry.iter())
        .map(|(_, name, value)| (name.clone(), value.clone()))
        .collect()
}

fn density_functions(registries: &Registries) -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
    named(registries)
}

/// Compiles one dimension's router and material rules from its noise settings
/// and the density functions, noises, material rules and conditions of
/// `registries`. A `biome_is` set resolves to the biome numbers of `registries`,
/// the numbering the column's biome grid holds.
///
/// `block` resolves a datapack block state against the block registry this
/// crate does not have; the terrain block and the sea fluid come from the settings
/// themselves, so they are per dimension rather than global.
pub fn build_dimension_router(
    registries: &Registries,
    settings: &NoiseGeneratorSettings,
    seed: u64,
    block: &dyn Fn(&BlockState) -> Option<VoxelId>,
) -> Result<(NoiseRouter, MaterialProgram), CompileError> {
    let resolve = |state: &BlockState| {
        block(state).ok_or_else(|| CompileError::UnknownBlockState(state.name.as_str().to_string()))
    };
    let stone = BlockState::from(Block::Stone);
    let blocks = RouterBlocks {
        default_block: resolve(settings.default_block.as_ref().unwrap_or(&stone))?,
        default_fluid: resolve(&settings.default_fluid)?,
        water: resolve(&Block::Water.into())?,
        lava: resolve(&Block::Lava.into())?,
    };

    let rules = named::<MaterialRuleHolder>(registries);
    let conditions = named::<MaterialConditionHolder>(registries);
    let biome_tags = registries.tags::<Biome>();
    let biomes = |set: &KeySet<Biome>| {
        let set = IdSet::<Biome>::resolve(set.clone(), registries).ok()?;
        Some(
            set.ids(biome_tags.as_ref()?)
                .map(|id| id.number())
                .collect(),
        )
    };
    let material = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block,
        biomes: &biomes,
    };
    build_router_and_material(
        settings,
        &density_functions(registries),
        &named::<NoiseParam>(registries),
        seed,
        blocks,
        &material,
    )
}

/// Where one dimension's `final_density` places the beardifier, with its
/// references resolved through the density functions of `registries`.
pub fn dimension_beardifier_placement(
    registries: &Registries,
    settings: &NoiseGeneratorSettings,
) -> BeardifierPlacement {
    beardifier_placement(
        &settings.noise_router.final_density,
        &density_functions(registries),
    )
}
