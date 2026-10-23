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
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, JukeboxSong, PaintingVariantValue, TrimMaterial, TrimPattern,
};
use mcrs_minecraft_registries::{Declaration, Resolve};
use mcrs_minecraft_sound::BlockSoundSet;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};
use mcrs_minecraft_worldgen_surface::proto::{MaterialConditionHolder, MaterialRuleHolder};

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

/// Every data registry whose values this crate reads; the data pack report stages each one.
pub(crate) fn typed_declarations() -> Vec<Declaration> {
    let mut declarations: Vec<Declaration> = vec![
        built_biomes(BiomeFile::declaration())
            .synced_via::<NetworkBiome>()
            .into(),
        Declaration::data::<Feature>().into(),
        Declaration::data::<PlacedFeature>().into(),
        Declaration::data::<StructureProcessorList>().into(),
        Declaration::data::<Structure>().into(),
        Declaration::data::<StructureSet>().into(),
        Declaration::data::<CarverConfig>().into(),
        built_template_pools(Declaration::data::<TemplatePool>()).into(),
        built_noises(Declaration::data::<NoiseParam>()).into(),
        built_density_functions(Declaration::data::<DensityFunctionHolder>()).into(),
        built_noise_settings(Declaration::data::<NoiseGeneratorSettings>()).into(),
        Declaration::data::<MaterialRuleHolder>().into(),
        Declaration::data::<MaterialConditionHolder>().into(),
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
            .synced_as(|file, registries| {
                let named = file.clone().synced().name(registries);
                let named = named.map_err(|error| error.to_string())?;
                mcrs_minecraft_nbt::to_nbt_tag(&named).map_err(|error| error.to_string())
            })
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

pub fn refuse<E: std::fmt::Display>(error: &E) -> ! {
    tracing::error!("the registries are unusable:\n{error}");
    std::process::exit(1)
}
