use crate::block_state::try_resolve_state;
use crate::plan_error::PlanError;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen::beard::BeardifierPlacement;
use mcrs_minecraft_worldgen::router::{build_dimension_router, dimension_beardifier_placement};
use mcrs_minecraft_worldgen_density::compile::CompileError;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen_structure::frozen::DimensionStructureTables;
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;

pub fn build_router(
    registries: &Registries,
    settings: &NoiseGeneratorSettings,
    seed: u64,
    blocks: &BlockDefinitions,
) -> Result<(NoiseRouter, MaterialProgram), CompileError> {
    let block = |state: &_| try_resolve_state(blocks, state).map(|state| state.number().into());
    build_dimension_router(registries, settings, seed, &block)
}

pub fn refuse_misplaced_beardifier(
    registries: &Registries,
    dimension: &ResourceLocationBuf,
    settings_name: &ResourceLocationBuf,
    settings: &NoiseGeneratorSettings,
    tables: &DimensionStructureTables,
) -> Result<(), PlanError> {
    if tables.live.is_empty() {
        return Ok(());
    }
    match (
        dimension_beardifier_placement(registries, settings),
        tables.adapted().next(),
    ) {
        (BeardifierPlacement::RootAddend, _) | (BeardifierPlacement::Absent, None) => Ok(()),
        (BeardifierPlacement::Misplaced, _) => Err(PlanError::BeardifierMisplaced {
            dimension: dimension.clone(),
            settings: settings_name.clone(),
        }),
        (BeardifierPlacement::Absent, Some(structure)) => Err(PlanError::BeardifierMissing {
            dimension: dimension.clone(),
            settings: settings_name.clone(),
            structure: structure.id.clone(),
        }),
    }
}
