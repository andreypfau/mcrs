use std::collections::{BTreeMap, BTreeSet};

use crate::chat_type::ChatType;
use crate::dimension::DimensionEntry;
use crate::sulfur_cube_archetype::SulfurCubeArchetype;
use crate::test_types::{TestEnvironment, TestInstance};
use crate::variant::{
    CatVariantFile, ChickenVariantFile, CowVariantFile, FrogVariantFile, PigVariantFile,
    WolfVariantFile, ZombieNautilusVariantFile,
};
use crate::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList;
use mcrs_minecraft_biome_file::{BiomeFile, NetworkBiome};
use mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_dimension::keys::DIMENSION;
use mcrs_minecraft_dimension_environment::dimension_type::{
    DimensionTypeFile, NetworkDimensionType,
};
use mcrs_minecraft_enchantment::file::EnchantmentFile;
use mcrs_minecraft_entity::variant::{
    CatSoundVariant, CatVariant, ChickenSoundVariant, ChickenVariant, CowSoundVariant, CowVariant,
    FrogVariant, PigSoundVariant, PigVariant, WolfSoundVariant, WolfVariant, ZombieNautilusVariant,
};
use mcrs_minecraft_environment::timeline::{NetworkTimeline, Timeline};
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_item::block_transformer::BlockTransformer;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern;
use mcrs_minecraft_item::dialog::Dialog;
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, JukeboxSong, PaintingVariantValue, TrimMaterial, TrimPattern,
};
use mcrs_minecraft_registries::{Declaration, Stage};
use mcrs_minecraft_sound::BlockSoundSet;
use serde::Deserialize;

use crate::registry_load::RegistryLoadError;

macro_rules! spawning_variants {
    ($($file:ident as $value:ident),+ $(,)?) => {
        [$(
            Declaration::data::<$file>()
                .non_empty()
                .synced_via::<$value>()
                .into(),
        )+]
    };
}

/// Every data registry whose values this crate reads.
fn typed_declarations() -> Vec<Declaration> {
    let mut declarations: Vec<Declaration> = vec![
        Declaration::data::<BiomeFile>()
            .synced_via::<NetworkBiome>()
            .into(),
        Declaration::data::<ChatType>().synced().into(),
        Declaration::data::<TrimPattern>().synced().into(),
        Declaration::data::<TrimMaterial>().synced().into(),
        Declaration::data::<WolfSoundVariant>()
            .non_empty()
            .synced()
            .into(),
        Declaration::data::<PigSoundVariant>()
            .non_empty()
            .synced()
            .into(),
        Declaration::data::<CatSoundVariant>()
            .non_empty()
            .synced()
            .into(),
        Declaration::data::<CowSoundVariant>()
            .non_empty()
            .synced()
            .into(),
        Declaration::data::<ChickenSoundVariant>()
            .non_empty()
            .synced()
            .into(),
        Declaration::data::<PaintingVariantValue>()
            .non_empty()
            .synced()
            .into(),
        Declaration::data::<SulfurCubeArchetype>().synced().into(),
        Declaration::data::<DimensionTypeFile>()
            .synced_via::<NetworkDimensionType>()
            .into(),
        Declaration::data::<DamageType>().synced().into(),
        Declaration::data::<BannerPattern>().synced().into(),
        Declaration::data::<EnchantmentFile>().synced().into(),
        Declaration::data::<JukeboxSong>().synced().into(),
        Declaration::data::<InstrumentValue>().synced().into(),
        Declaration::data::<TestEnvironment>().synced().into(),
        Declaration::data::<TestInstance>().synced().into(),
        Declaration::data::<Dialog>().synced().into(),
        Declaration::data::<WorldClock>().synced().into(),
        Timeline::declaration()
            .synced_via::<NetworkTimeline>()
            .into(),
        Declaration::data::<DecoratedPotPattern>().synced().into(),
        Declaration::data::<BlockTransformer>().synced().into(),
        Declaration::data::<DirectBlockStateProvider>()
            .synced()
            .into(),
        Declaration::data::<BlockSoundSet>().synced().into(),
        Declaration::data::<DimensionEntry>().into(),
        Declaration::data::<MultiNoiseBiomeSourceParameterList>().into(),
        Declaration::data::<WorldPreset>().into(),
    ];
    declarations.extend(spawning_variants!(
        WolfVariantFile as WolfVariant,
        PigVariantFile as PigVariant,
        FrogVariantFile as FrogVariant,
        CatVariantFile as CatVariant,
        CowVariantFile as CowVariant,
        ChickenVariantFile as ChickenVariant,
        ZombieNautilusVariantFile as ZombieNautilusVariant,
    ));
    declarations
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
fn data_registries(report: &[u8]) -> Result<BTreeMap<ResourceLocation, Stage>, RegistryLoadError> {
    let report: DatapackReport = serde_json::from_slice(report).map_err(|error| {
        RegistryLoadError::Declarations(format!("the data pack report: {error}"))
    })?;
    Ok(report
        .registries
        .into_iter()
        .filter(|(_, flags)| flags.elements)
        .map(|(key, flags)| {
            let stage = match (flags.stable, key == DIMENSION.location()) {
                (true, _) => Stage::Reloadable,
                (false, true) => Stage::Dimension,
                (false, false) => Stage::World,
            };
            (key, stage)
        })
        .collect())
}

/// Every registry a load declares: the static registries, the data registries this crate reads,
/// `extra`, and every other data registry of the data pack `report` by its names alone. A
/// declaration of `extra` replaces the declaration of this crate for its key. A data
/// registry that the report lists loads in the stage of the report; any other keeps its own.
pub fn declarations(
    report: &[u8],
    extra: Vec<Declaration>,
) -> Result<Vec<Declaration>, RegistryLoadError> {
    let mut declarations = Vec::new();
    for (key, names) in mcrs_minecraft_registry_catalog::STATIC_REGISTRIES {
        let names = names.iter().map(|&name| name.into()).collect();
        declarations.push(Declaration::fixed((*key).into(), names));
    }

    let mut stages = data_registries(report)?;
    let replaced: BTreeSet<ResourceLocation> = extra
        .iter()
        .map(|declaration| declaration.key().clone())
        .collect();
    let typed = typed_declarations()
        .into_iter()
        .filter(|declaration| !replaced.contains(declaration.key()));
    for declaration in typed.chain(extra) {
        declarations.push(match stages.remove(declaration.key()) {
            Some(stage) => declaration.in_stage(stage),
            None => declaration,
        });
    }

    declarations.extend(
        stages
            .into_iter()
            .map(|(key, stage)| Declaration::names_only(key, stage)),
    );
    Ok(declarations)
}
