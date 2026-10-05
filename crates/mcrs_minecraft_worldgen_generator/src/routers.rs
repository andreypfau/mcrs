use crate::block_state::try_resolve_state;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Id, Registry};
use mcrs_minecraft_worldgen::beard::BeardifierPlacement;
use mcrs_minecraft_worldgen::bevy::{
    NoiseGeneratorSettingsAsset, WorldgenAssets, build_dimension_router,
    dimension_beardifier_placement,
};
use mcrs_minecraft_worldgen_density::compile::CompileError;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_structure::frozen::DimensionStructureTables;
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;

/// The material rules take their biome ids from the registry, because it is
/// what `MultiNoiseBiomeTable` fills the column's biome grid with.
pub fn build_router(
    settings: &NoiseGeneratorSettingsAsset,
    assets: &WorldgenAssets<'_>,
    seed: u64,
    blocks: &BlockDefinitions,
    biomes: &Registry<keys::Biome>,
) -> Result<(NoiseRouter, MaterialProgram), CompileError> {
    let block = |state: &_| try_resolve_state(blocks, state).map(Into::into);
    let biome = |id: &ResourceLocation| biomes.get(id.as_str()).map(Id::number);
    build_dimension_router(settings, assets, seed, &block, &biome)
}

/// The fill adds the beard to `final_density` once the graph is evaluated,
/// which is the graph's own value only while the beardifier is the outermost
/// addend, so any other shape would silently lose the term.
pub fn refuse_misplaced_beardifier(
    dimension: &ResourceLocation,
    settings_name: &ResourceLocation,
    settings: &NoiseGeneratorSettingsAsset,
    assets: &WorldgenAssets<'_>,
    tables: &DimensionStructureTables,
) {
    if tables.live.is_empty() {
        return;
    }
    match (
        dimension_beardifier_placement(settings, assets),
        tables.adapted().next(),
    ) {
        (BeardifierPlacement::RootAddend, _) | (BeardifierPlacement::Absent, None) => {}
        (BeardifierPlacement::Misplaced, _) => panic!(
            "{dimension}: {settings_name} uses minecraft:beardifier other than as an operand of the add at the root of final_density"
        ),
        (BeardifierPlacement::Absent, Some(structure)) => panic!(
            "{dimension}: the final_density of {settings_name} is not add(_, minecraft:beardifier), so {} cannot adapt the terrain",
            structure.id
        ),
    }
}
