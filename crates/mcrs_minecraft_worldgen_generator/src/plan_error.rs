use mcrs_minecraft_registries::Diagnostics;
use mcrs_minecraft_resource::{Id, RegistryValue, ResourceLocationBuf};
use mcrs_minecraft_worldgen_density::compile::CompileError;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::compile::FeatureCompileError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PlanError {
    #[error("the dimension has no generator that fills columns")]
    NoColumnGenerator,
    #[error("the registries declare no noise settings registry")]
    NoNoiseSettingsRegistry,
    #[error("the dimension names noise settings the registry does not number: {settings:?}")]
    UnknownNoiseSettings {
        settings: Id<<NoiseGeneratorSettings as RegistryValue>::Registry>,
    },
    #[error("the noise settings did not compile: {0}")]
    RouterCompile(CompileError),
    #[error("the registries lack a name worldgen needs: {0}")]
    MissingNames(Diagnostics),
    #[error("the biome {0} has no loaded definition")]
    UnloadedBiome(ResourceLocationBuf),
    #[error("the feature steps do not resolve: {0}")]
    FeatureSteps(FeatureCompileError),
    #[error("the feature program does not resolve: {0}")]
    FeatureProgram(FeatureCompileError),
    #[error("{dimension}: mcrs:beta_cave carves only over a Beta biome source")]
    BetaCaveOutsideBetaSource { dimension: ResourceLocationBuf },
    #[error(
        "{dimension}: {settings} uses minecraft:beardifier other than as an operand of the add at the root of final_density"
    )]
    BeardifierMisplaced {
        dimension: ResourceLocationBuf,
        settings: ResourceLocationBuf,
    },
    #[error(
        "{dimension}: the final_density of {settings} is not add(_, minecraft:beardifier), so {structure} cannot adapt the terrain"
    )]
    BeardifierMissing {
        dimension: ResourceLocationBuf,
        settings: ResourceLocationBuf,
        structure: ResourceLocationBuf,
    },
}

impl PlanError {
    #[must_use]
    pub fn is_invalid_data_pack(&self) -> bool {
        match self {
            PlanError::UnloadedBiome(_)
            | PlanError::FeatureSteps(_)
            | PlanError::FeatureProgram(_)
            | PlanError::BetaCaveOutsideBetaSource { .. }
            | PlanError::BeardifierMisplaced { .. }
            | PlanError::BeardifierMissing { .. } => true,
            PlanError::NoColumnGenerator
            | PlanError::NoNoiseSettingsRegistry
            | PlanError::UnknownNoiseSettings { .. }
            | PlanError::RouterCompile(_)
            | PlanError::MissingNames(_) => false,
        }
    }
}
