use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::VANILLA_PACK;
use mcrs_minecraft_registries::{DataDeclaration, DataValue, Declaration, Resolve, Stage};
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::Deserialize;

use crate::registries::typed_declarations;

pub(crate) const BETA_PACK: &str = "file/beta";

const DIMENSION: &str = "minecraft:dimension";

struct Taken<V>(Mutex<BTreeMap<ResourceLocation, V>>);

impl<V> Taken<V> {
    fn new(values: BTreeMap<ResourceLocation, V>) -> Self {
        Self(Mutex::new(values))
    }

    fn names(&self) -> Vec<ResourceLocation> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }

    fn take(&self, name: &ResourceLocation) -> Option<V> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(name)
    }
}

fn with_built_ins<V: DataValue + Resolve<Named = V>>(
    declaration: DataDeclaration<V>,
    vanilla: BTreeMap<ResourceLocation, V>,
    beta: BTreeMap<ResourceLocation, V>,
) -> DataDeclaration<V> {
    let (vanilla, beta) = (Taken::new(vanilla), Taken::new(beta));
    let (vanilla_names, beta_names) = (vanilla.names(), beta.names());
    declaration
        .built_in(VANILLA_PACK, vanilla_names, move |name| vanilla.take(name))
        .built_in(BETA_PACK, beta_names, move |name| beta.take(name))
}

pub(crate) fn built_biomes(declaration: DataDeclaration<BiomeFile>) -> DataDeclaration<BiomeFile> {
    declaration.built_in(VANILLA_PACK, builtin::biome_names(), builtin::biome)
}

pub(crate) fn built_template_pools(
    declaration: DataDeclaration<TemplatePool>,
) -> DataDeclaration<TemplatePool> {
    with_built_ins(declaration, builtin::template_pools(), BTreeMap::new())
}

pub(crate) fn built_noises(
    declaration: DataDeclaration<NoiseParam>,
) -> DataDeclaration<NoiseParam> {
    with_built_ins(
        declaration,
        builtin::vanilla_noises(),
        builtin::beta_noises(),
    )
}

pub(crate) fn built_density_functions(
    declaration: DataDeclaration<DensityFunctionHolder>,
) -> DataDeclaration<DensityFunctionHolder> {
    with_built_ins(
        declaration,
        builtin::vanilla_density_functions(),
        builtin::beta_density_functions(),
    )
}

pub(crate) fn built_noise_settings(
    declaration: DataDeclaration<NoiseGeneratorSettings>,
) -> DataDeclaration<NoiseGeneratorSettings> {
    with_built_ins(
        declaration,
        builtin::vanilla_noise_settings(),
        builtin::beta_noise_settings(),
    )
}

#[derive(Deserialize)]
struct DatapackReport {
    registries: BTreeMap<ResourceLocation, Flags>,
}

#[derive(Deserialize)]
struct Flags {
    elements: bool,
    stable: bool,
}

/// The stage of every data registry the data pack report lists: one whose entries a reload may
/// change is reloadable.
fn data_registries(report: &[u8]) -> Result<BTreeMap<ResourceLocation, Stage>, String> {
    let report: DatapackReport =
        serde_json::from_slice(report).map_err(|error| format!("the data pack report: {error}"))?;
    Ok(report
        .registries
        .into_iter()
        .filter(|(_, flags)| flags.elements)
        .map(|(key, flags)| {
            let stage = match (flags.stable, key.as_str()) {
                (true, _) => Stage::Reloadable,
                (false, DIMENSION) => Stage::Dimension,
                (false, _) => Stage::World,
            };
            (key, stage)
        })
        .collect())
}

/// Every registry a load declares: the static registries, the data registries this crate reads
/// and every other data registry of the report by its names alone.
pub(crate) fn plan(report: &[u8]) -> Result<Vec<Declaration>, String> {
    let mut declarations = Vec::new();
    for (key, names) in mcrs_minecraft_registry_catalog::STATIC_REGISTRIES {
        let names = names.iter().map(|&name| name.into()).collect();
        declarations.push(Declaration::fixed((*key).into(), names));
    }

    let mut stages = data_registries(report)?;
    for declaration in typed_declarations() {
        let stage = stages.remove(declaration.key()).ok_or_else(|| {
            format!(
                "{} is not a data registry of the data pack report",
                declaration.key()
            )
        })?;
        declarations.push(declaration.in_stage(stage));
    }

    declarations.extend(
        stages
            .into_iter()
            .map(|(key, stage)| Declaration::names_only(key, stage)),
    );
    Ok(declarations)
}
