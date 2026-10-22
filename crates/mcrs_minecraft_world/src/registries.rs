use crate::chat_type::ChatType;
use crate::declarations::{
    built_biomes, built_density_functions, built_noise_settings, built_noises, built_template_pools,
};
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
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
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
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, JukeboxSong, PaintingVariantValue, TrimMaterial, TrimPattern,
};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::{
    DataDeclaration, Declaration, ReadValue, Registries, RegistryOf, Resolve, Stage,
};
use mcrs_minecraft_sound::BlockSoundSet;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};
use mcrs_minecraft_worldgen_surface::proto::{MaterialConditionHolder, MaterialRuleHolder};
use serde::Serialize;

fn named_tag<T: Resolve<Named: Serialize>>(
    value: &T,
    registries: &Registries,
) -> Result<NbtTag, String> {
    nbt_tag(&value.name(registries).map_err(|error| error.to_string())?)
}

fn from_tag<T: serde::de::DeserializeOwned>(tag: NbtTag) -> Result<T, String> {
    mcrs_minecraft_nbt::from_tag(tag).map_err(|error| error.to_string())
}

fn nbt_tag<T: Serialize>(value: &T) -> Result<NbtTag, String> {
    mcrs_minecraft_nbt::to_nbt_tag(value).map_err(|error| error.to_string())
}

/// A registry sent to clients in its named form, read back as its file is.
fn synced_named<V>(declaration: DataDeclaration<RegistryOf<V>>) -> DataDeclaration<RegistryOf<V>>
where
    V: ReadValue + Resolve<Named: Serialize> + Send + Sync + 'static,
{
    declaration.synced(named_tag)
}

/// A registry sent to clients as its file is written: it names no entry.
fn synced_plain<V>(declaration: DataDeclaration<RegistryOf<V>>) -> DataDeclaration<RegistryOf<V>>
where
    V: ReadValue + Serialize + Send + Sync + 'static,
{
    declaration.synced(|value, _| nbt_tag(value))
}

/// A declaration with the key and the stage the data pack report must list it under.
pub(crate) struct Typed {
    pub(crate) key: ResourceLocation,
    pub(crate) stage: Stage,
    pub(crate) declaration: Declaration,
}

impl<V: ReadValue + Send + Sync + 'static> From<DataDeclaration<RegistryOf<V>>> for Typed {
    fn from(declaration: DataDeclaration<RegistryOf<V>>) -> Self {
        Typed {
            key: declaration.key().clone(),
            stage: declaration.stage(),
            declaration: declaration.into(),
        }
    }
}

macro_rules! spawning_variants {
    ($($file:ident as $value:ident),+ $(,)?) => {
        [$(
            $file::declaration()
                .named_by::<$value>()
                .synced(|file, _| nbt_tag(&$file::split(file).0))
                .received(|tag, _| {
                    let value: $value = from_tag(tag)?;
                    Ok($file::join((&value, &Vec::new())))
                })
                .into(),
        )+]
    };
}

/// Every data registry whose values this crate reads, keyed and staged as the data pack report
/// lists it.
pub(crate) fn typed_declarations() -> Vec<Typed> {
    let mut declarations: Vec<Typed> = vec![
        built_biomes(BiomeFile::declaration())
            .synced(|biome, _| {
                let (biome, attributes, generation) = biome.split();
                nbt_tag(&NetworkBiome::from((&biome, &attributes, &generation)))
            })
            .received(|tag, _| {
                let parts = from_tag::<NetworkBiome>(tag)?.into_client_parts();
                Ok(BiomeFile::join((&parts.0, &parts.1, &parts.2)))
            })
            .into(),
        Feature::declaration().into(),
        PlacedFeature::declaration().into(),
        StructureProcessorList::declaration().into(),
        Structure::declaration().into(),
        StructureSet::declaration().into(),
        CarverConfig::declaration().into(),
        built_template_pools(TemplatePool::declaration()).into(),
        built_noises(NoiseParam::declaration()).into(),
        built_density_functions(DensityFunctionHolder::declaration()).into(),
        built_noise_settings(NoiseGeneratorSettings::declaration()).into(),
        MaterialRuleHolder::declaration().into(),
        MaterialConditionHolder::declaration().into(),
        synced_named(ChatType::declaration()).into(),
        synced_named(TrimPattern::declaration().named_by::<TrimPattern>()).into(),
        synced_named(TrimMaterial::declaration().named_by::<TrimMaterial>()).into(),
        synced_named(WolfSoundVariant::declaration().named_by::<WolfSoundVariant>()).into(),
        synced_named(PigSoundVariant::declaration().named_by::<PigSoundVariant>()).into(),
        synced_named(CatSoundVariant::declaration().named_by::<CatSoundVariant>()).into(),
        synced_named(CowSoundVariant::declaration().named_by::<CowSoundVariant>()).into(),
        synced_named(ChickenSoundVariant::declaration().named_by::<ChickenSoundVariant>()).into(),
        synced_named(PaintingVariantValue::declaration().named_by::<PaintingVariantValue>()).into(),
        synced_named(SulfurCubeArchetype::declaration()).into(),
        DimensionTypeFile::declaration()
            .named_by::<DimensionType>()
            .synced(|file, registries| named_tag(&file.clone().synced(), registries))
            .into(),
        synced_plain(DamageType::declaration()).into(),
        synced_plain(BannerPattern::declaration()).into(),
        synced_named(EnchantmentFile::declaration().named_by::<EnchantmentData>()).into(),
        synced_named(JukeboxSong::declaration().named_by::<JukeboxSong>()).into(),
        synced_named(InstrumentValue::declaration().named_by::<InstrumentValue>()).into(),
        synced_named(TestEnvironment::declaration()).into(),
        synced_named(TestInstance::declaration()).into(),
        synced_named(Dialog::declaration().named_by::<Dialog>()).into(),
        synced_plain(WorldClock::declaration()).into(),
        Timeline::declaration()
            .synced(|timeline, registries| named_tag(&NetworkTimeline::from(timeline), registries))
            .received(|tag, ctx| {
                let named = from_tag(tag)?;
                let timeline = NetworkTimeline::resolve(named, ctx).map_err(|e| e.to_string())?;
                Ok(Timeline::from(timeline))
            })
            .into(),
        synced_plain(DecoratedPotPattern::declaration()).into(),
        synced_named(BlockTransformer::declaration()).into(),
        synced_named(DirectBlockStateProvider::declaration()).into(),
        synced_named(BlockSoundSet::declaration()).into(),
        DimensionEntry::declaration().into(),
        MultiNoiseBiomeSourceParameterList::declaration()
            .named_by::<MultiNoiseBiomeSourceParameterList>()
            .into(),
        WorldPreset::declaration().into(),
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

pub fn refuse<E: std::fmt::Display>(error: &E) -> ! {
    tracing::error!("the registries are unusable:\n{error}");
    std::process::exit(1)
}
