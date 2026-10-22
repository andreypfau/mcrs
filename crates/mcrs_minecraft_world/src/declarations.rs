use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::keys::{BLOCK, BLOCK_ENTITY_TYPE, Block, BlockEntityType, FLUID, Fluid};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::resource_location::StaticResourceLocation;
use mcrs_minecraft_entity::keys::{
    ATTRIBUTE, Attribute, ENTITY_TYPE, EntityType, VILLAGER_TYPE, VillagerType,
};
use mcrs_minecraft_item::keys::{ITEM, Item, MOB_EFFECT, MobEffect, POTION, Potion};
use mcrs_minecraft_registries::{
    DataDeclaration, Declaration, ReadValue, RegistryOf, Resolve, Stage, VANILLA_PACK,
};
use mcrs_minecraft_sound::SoundEvent;
use mcrs_minecraft_sound::keys::SOUND_EVENT;
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::Deserialize;

use crate::registries::typed_declarations;

pub(crate) const BETA_PACK: &str = "file/beta";

const DIMENSION: &str = "minecraft:dimension";

type DeclareStatic = fn(ResourceLocation, Vec<ResourceLocation>) -> Declaration;

const TYPED_STATICS: [(StaticResourceLocation, DeclareStatic); 10] = [
    (BLOCK.location(), Declaration::fixed::<Block>),
    (
        BLOCK_ENTITY_TYPE.location(),
        Declaration::fixed::<BlockEntityType>,
    ),
    (ITEM.location(), Declaration::fixed::<Item>),
    (FLUID.location(), Declaration::fixed::<Fluid>),
    (SOUND_EVENT.location(), Declaration::fixed::<SoundEvent>),
    (ATTRIBUTE.location(), Declaration::fixed::<Attribute>),
    (MOB_EFFECT.location(), Declaration::fixed::<MobEffect>),
    (POTION.location(), Declaration::fixed::<Potion>),
    (ENTITY_TYPE.location(), Declaration::fixed::<EntityType>),
    (VILLAGER_TYPE.location(), Declaration::fixed::<VillagerType>),
];

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

    fn take(&self, name: &ResourceLocation) -> Result<V, String> {
        let mut values = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        values
            .remove(name)
            .ok_or_else(|| format!("no built-in value is named {name}"))
    }
}

fn with_built_ins<V: ReadValue + Send + Sync + 'static>(
    declaration: DataDeclaration<RegistryOf<V>>,
    vanilla: BTreeMap<ResourceLocation, V>,
    beta: BTreeMap<ResourceLocation, V>,
) -> DataDeclaration<RegistryOf<V>> {
    let (vanilla, beta) = (Taken::new(vanilla), Taken::new(beta));
    let (vanilla_names, beta_names) = (vanilla.names(), beta.names());
    declaration
        .built_in(VANILLA_PACK, vanilla_names, move |name, _| {
            vanilla.take(name)
        })
        .built_in(BETA_PACK, beta_names, move |name, _| beta.take(name))
}

pub(crate) fn built_biomes(
    declaration: DataDeclaration<RegistryOf<BiomeFile>>,
) -> DataDeclaration<RegistryOf<BiomeFile>> {
    declaration.built_in(VANILLA_PACK, builtin::biome_names(), |name, ctx| {
        let draft =
            builtin::biome(name).ok_or_else(|| format!("no built-in biome is named {name}"))?;
        BiomeFile::resolve(draft, ctx).map_err(|error| error.to_string())
    })
}

pub(crate) fn built_template_pools(
    declaration: DataDeclaration<RegistryOf<TemplatePool>>,
) -> DataDeclaration<RegistryOf<TemplatePool>> {
    with_built_ins(declaration, builtin::template_pools(), BTreeMap::new())
}

pub(crate) fn built_noises(
    declaration: DataDeclaration<RegistryOf<NoiseParam>>,
) -> DataDeclaration<RegistryOf<NoiseParam>> {
    with_built_ins(
        declaration,
        builtin::vanilla_noises(),
        builtin::beta_noises(),
    )
}

pub(crate) fn built_density_functions(
    declaration: DataDeclaration<RegistryOf<DensityFunctionHolder>>,
) -> DataDeclaration<RegistryOf<DensityFunctionHolder>> {
    with_built_ins(
        declaration,
        builtin::vanilla_density_functions(),
        builtin::beta_density_functions(),
    )
}

pub(crate) fn built_noise_settings(
    declaration: DataDeclaration<RegistryOf<NoiseGeneratorSettings>>,
) -> DataDeclaration<RegistryOf<NoiseGeneratorSettings>> {
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
        let typed = TYPED_STATICS
            .iter()
            .find(|(location, _)| key.as_static_str() == location.as_static_str());
        declarations.push(match typed {
            Some((_, declare)) => declare((*key).into(), names),
            None => Declaration::fixed_names((*key).into(), names),
        });
    }

    let mut stages = data_registries(report)?;
    for typed in typed_declarations() {
        let stage = stages.remove(&typed.key).ok_or_else(|| {
            format!(
                "{} is not a data registry of the data pack report",
                typed.key
            )
        })?;
        if stage != typed.stage {
            return Err(format!(
                "{} is declared for the {:?} stage and the data pack report loads it in the {stage:?} stage",
                typed.key, typed.stage
            ));
        }
        declarations.push(typed.declaration);
    }

    declarations.extend(
        stages
            .into_iter()
            .map(|(key, stage)| Declaration::names_only(key, stage)),
    );
    Ok(declarations)
}
