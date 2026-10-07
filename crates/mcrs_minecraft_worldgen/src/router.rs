use crate::beard::{BeardifierPlacement, beardifier_placement};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::{Registered, RegistrySet, Tags};
use mcrs_minecraft_worldgen_density::compile::CompileError;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter, RouterBlocks};
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_surface::compile::{MaterialProgram, build_router_and_material};
use mcrs_minecraft_worldgen_surface::proto::MaterialRule;
use mcrs_minecraft_worldgen_surface::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
};
use std::collections::BTreeMap;

// chisle: the compiler takes its registries keyed by name, so every entry of
// each is cloned once per dimension at start. A compiler that takes ids and
// reads the set's columns lifts it.
fn named<R: Registered, T: Clone + 'static>(set: &RegistrySet) -> BTreeMap<ResourceLocation, T> {
    let registry = set
        .registry::<R>()
        .unwrap_or_else(|| panic!("the registry set declares {}", R::REGISTRY));
    let values = set.entries::<R, T>().unwrap_or_else(|| {
        panic!(
            "the registry set parses {} into {}",
            R::REGISTRY,
            std::any::type_name::<T>()
        )
    });
    registry
        .ids()
        .map(|id| {
            let name = registry.name(id).expect("an id of the registry has a name");
            let value = values.get(id).expect("a value for every id");
            (name.clone(), value.clone())
        })
        .collect()
}

fn density_functions(set: &RegistrySet) -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
    named::<DensityFunctionHolder, DensityFunctionHolder>(set)
}

/// Compiles one dimension's router and material rules from its noise settings
/// and the density functions, noises, material rules and conditions of `set`.
///
/// `block` resolves a datapack block state against the block registry this
/// crate does not have, and `biome_tags` holds the biome tags a `biome_is` set
/// may name, whose ids are the numbering the column's biome grid holds. Both
/// are the caller's; the terrain block and the sea fluid come from the settings
/// themselves, so they are per dimension rather than global.
pub fn build_dimension_router(
    set: &RegistrySet,
    settings: &NoiseGeneratorSettings,
    seed: u64,
    block: &dyn Fn(&BlockState) -> Option<VoxelId>,
    biome_tags: &Tags<mcrs_minecraft_biome::Biome>,
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

    let rules = named::<MaterialRule, MaterialRuleHolder>(set);
    let conditions = named::<MaterialConditionHolder, MaterialConditionHolder>(set);
    let material = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block,
        biome_tags,
    };
    build_router_and_material(
        settings,
        &density_functions(set),
        &named::<NoiseParam, NoiseParam>(set),
        seed,
        blocks,
        &material,
    )
}

/// Where one dimension's `final_density` places the beardifier, with its
/// references resolved through the density functions of `set`.
pub fn dimension_beardifier_placement(
    set: &RegistrySet,
    settings: &NoiseGeneratorSettings,
) -> BeardifierPlacement {
    beardifier_placement(
        &settings.noise_router.final_density,
        &density_functions(set),
    )
}
