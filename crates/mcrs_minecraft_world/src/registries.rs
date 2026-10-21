use crate::chat_type::ChatType;
use crate::declarations::{
    Planner, StageSet, built_biomes, built_density_functions, built_noise_settings, built_noises,
    built_template_pools,
};
use crate::dimension::DimensionEntry;
use crate::enchantment_provider::EnchantmentProvider;
use crate::sulfur_cube_archetype::SulfurCubeArchetype;
use crate::test_types::{TestEnvironment, TestInstance};
use crate::variant::{
    CatVariantFile, ChickenVariantFile, CowVariantFile, FrogVariantFile, PigVariantFile,
    SpawnSelector, WolfVariantFile, ZombieNautilusVariantFile,
};
use crate::villager_trade::{TradeSet, VillagerTrade};
use crate::worldgen::chunk_generator::ChunkGenerator;
use crate::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList;
use mcrs_minecraft_biome_file::{BiomeFile, BiomeGenerationSettings, NetworkBiome};
use mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_dimension_environment::dimension_type::{
    DimensionTypeEnvironment, DimensionTypeFile,
};
use mcrs_minecraft_enchantment::effects::EnchantmentEffects;
use mcrs_minecraft_enchantment::file::EnchantmentFile;
use mcrs_minecraft_entity::variant::{
    CatSoundVariant, CatVariant, ChickenSoundVariant, ChickenVariant, CowSoundVariant, CowVariant,
    FrogVariant, PigSoundVariant, PigVariant, WolfSoundVariant, WolfVariant, ZombieNautilusVariant,
};
use mcrs_minecraft_environment::attribute::EnvironmentAttributeMap;
use mcrs_minecraft_environment::timeline::{NetworkTimeline, Timeline};
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_item::block_transformer::BlockTransformer;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern;
use mcrs_minecraft_item::dialog::Dialog;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::loot::{ContextFloatProvider, ContextIntProvider, LootTable};
use mcrs_minecraft_item::recipe::Recipe;
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, JukeboxSong, PaintingVariantValue, TrimMaterial, TrimPattern,
};
use mcrs_minecraft_loot::number::{FloatExpression, IntExpression};
use mcrs_minecraft_loot::{
    LootCondition, LootItemFunction, LootTableBody, LootTableFile, SlotSource,
};
use mcrs_minecraft_registries::{DataDeclaration, Declaration, RegistryOf};
use mcrs_minecraft_registry::{LoadReport, RegistrySet, WorldRegistries};
use mcrs_minecraft_sound::BlockSoundSet;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};
use mcrs_minecraft_worldgen_surface::proto::{
    MaterialConditionHolder, MaterialRule, MaterialRuleHolder,
};
use serde::Serialize;

macro_rules! registry_table {
    (
        fn $declare:ident, fn $declarations:ident, |$scope:ident, $stage:ident|
        $(
            $(@$mode:ident)? $key:ty => $value:ty
                $(as $parts:ty, $split:expr, $join:expr
                    $(, synced from parts as $part_project:expr, received as $part_net:ty => $part_receive:expr)?)?
                $(, synced as $project:expr, received as $net:ty => $receive:expr)?
                , declared by $declaration:expr $(, built in $built_in:expr)?;
        )*
    ) => {
        fn $declare<S>(world: &mut WorldRegistries, report: &mut LoadReport, $scope: S)
        where
            S: Fn() -> RegistrySet + Clone + Send + Sync + 'static,
        {
            $(
                declare_row!($($mode)?; $key, $value, world, report, $declaration);
                let registry = <$key as mcrs_minecraft_core::Registered>::REGISTRY.location();
                if world.parses(registry.as_static_str()) {
                    $(world.split::<$value, $parts>(registry, $split, $join);)?
                    $($(
                        world.sync_parts::<$parts, _>(registry, $part_project);
                        world.receive::<$part_net, _>(registry, $part_receive);
                    )?)?
                    $(
                        world.sync_value::<$value, _>(registry, $project);
                        world.receive::<$net, _>(registry, $receive);
                    )?
                }
            )*
        }

        pub(crate) fn $declarations(
            planner: &mut Planner,
            $stage: &StageSet,
        ) -> Result<Vec<Declaration>, String> {
            let $scope = $stage.scope();
            Ok(vec![$({
                let declaration = $declaration;
                $(let declaration = $built_in(declaration, $stage);)?
                planner.add(declaration)?
            }),*])
        }
    };
}

macro_rules! declare_row {
    (synced; $key:ty, $value:ty, $world:ident, $report:ident, $declaration:expr) => {
        let declaration = $declaration;
        if declared::<$key, $value>($world, $report, &declaration) {
            let registry = <$key as mcrs_minecraft_core::Registered>::REGISTRY.location();
            $world.parse_named::<$value>(registry);
            if declaration.is_non_empty() {
                $world.non_empty(registry);
            }
            $world.sync_named::<$value>(registry);
        }
    };
    (; $key:ty, $value:ty, $world:ident, $report:ident, $declaration:expr) => {
        declare::<$key, $value>($world, $report, $declaration);
    };
}

fn bare<T>(value: T) -> (T,) {
    (value,)
}

registry_table! {
    fn declare_world_registries, fn world_declarations, |scope, stage|
    Biome => BiomeFile
        as (Biome, EnvironmentAttributeMap, BiomeGenerationSettings),
        BiomeFile::split, BiomeFile::join,
        synced from parts as |parts| NetworkBiome::from(parts),
        received as NetworkBiome => NetworkBiome::into_parts,
        declared by BiomeFile::declaration(scope.clone()), built in built_biomes;
    Feature => Feature, declared by Feature::declaration();
    PlacedFeature => PlacedFeature, declared by PlacedFeature::declaration();
    StructureProcessorList => StructureProcessorList, declared by StructureProcessorList::declaration();
    Structure => Structure, declared by Structure::declaration();
    StructureSet => StructureSet, declared by StructureSet::declaration();
    CarverConfig => CarverConfig, declared by CarverConfig::declaration();
    TemplatePool => TemplatePool, declared by TemplatePool::declaration(), built in built_template_pools;
    NoiseParam => NoiseParam, declared by NoiseParam::declaration(), built in built_noises;
    DensityFunctionHolder => DensityFunctionHolder,
        declared by DensityFunctionHolder::declaration(), built in built_density_functions;
    NoiseGeneratorSettings => NoiseGeneratorSettings,
        declared by NoiseGeneratorSettings::declaration(), built in built_noise_settings;
    MaterialRule => MaterialRuleHolder, declared by MaterialRuleHolder::declaration();
    MaterialConditionHolder => MaterialConditionHolder, declared by MaterialConditionHolder::declaration();
    crate::chat_type::ChatType => ChatType, synced as Clone::clone, received as ChatType => bare,
        declared by ChatType::declaration();
    mcrs_minecraft_item::TrimPattern => TrimPattern, synced as Clone::clone, received as TrimPattern => bare,
        declared by TrimPattern::declaration();
    mcrs_minecraft_item::TrimMaterial => TrimMaterial, synced as Clone::clone, received as TrimMaterial => bare,
        declared by TrimMaterial::declaration();
    WolfVariant => WolfVariantFile
        as (WolfVariant, Vec<SpawnSelector>),
        WolfVariantFile::split, WolfVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as WolfVariant => bare,
        declared by WolfVariantFile::declaration();
    @synced WolfSoundVariant => WolfSoundVariant,
        declared by WolfSoundVariant::declaration();
    PigVariant => PigVariantFile
        as (PigVariant, Vec<SpawnSelector>),
        PigVariantFile::split, PigVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as PigVariant => bare,
        declared by PigVariantFile::declaration();
    @synced PigSoundVariant => PigSoundVariant,
        declared by PigSoundVariant::declaration();
    FrogVariant => FrogVariantFile
        as (FrogVariant, Vec<SpawnSelector>),
        FrogVariantFile::split, FrogVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as FrogVariant => bare,
        declared by FrogVariantFile::declaration();
    CatVariant => CatVariantFile
        as (CatVariant, Vec<SpawnSelector>),
        CatVariantFile::split, CatVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as CatVariant => bare,
        declared by CatVariantFile::declaration();
    @synced CatSoundVariant => CatSoundVariant,
        declared by CatSoundVariant::declaration();
    CowVariant => CowVariantFile
        as (CowVariant, Vec<SpawnSelector>),
        CowVariantFile::split, CowVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as CowVariant => bare,
        declared by CowVariantFile::declaration();
    @synced CowSoundVariant => CowSoundVariant,
        declared by CowSoundVariant::declaration();
    ChickenVariant => ChickenVariantFile
        as (ChickenVariant, Vec<SpawnSelector>),
        ChickenVariantFile::split, ChickenVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as ChickenVariant => bare,
        declared by ChickenVariantFile::declaration();
    @synced ChickenSoundVariant => ChickenSoundVariant,
        declared by ChickenSoundVariant::declaration();
    ZombieNautilusVariant => ZombieNautilusVariantFile
        as (ZombieNautilusVariant, Vec<SpawnSelector>),
        ZombieNautilusVariantFile::split, ZombieNautilusVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as ZombieNautilusVariant => bare,
        declared by ZombieNautilusVariantFile::declaration();
    mcrs_minecraft_item::PaintingVariantValue => PaintingVariantValue, synced as Clone::clone,
        received as PaintingVariantValue => bare,
        declared by PaintingVariantValue::declaration();
    @synced crate::sulfur_cube_archetype::SulfurCubeArchetype => SulfurCubeArchetype,
        declared by SulfurCubeArchetype::declaration();
    DimensionType => DimensionTypeFile
        as (DimensionType, DimensionTypeEnvironment),
        DimensionTypeFile::split, DimensionTypeFile::join,
        synced from parts as |parts| DimensionTypeFile::join(parts).synced(),
        received as DimensionTypeFile => |file| file.split(),
        declared by DimensionTypeFile::declaration();
    mcrs_minecraft_item::damage_type::DamageType => DamageType, synced as Clone::clone, received as DamageType => bare,
        declared by DamageType::declaration();
    mcrs_minecraft_item::BannerPattern => BannerPattern, synced as Clone::clone, received as BannerPattern => bare,
        declared by BannerPattern::declaration();
    mcrs_minecraft_item::enchantment::EnchantmentData => EnchantmentFile
        as (EnchantmentData, Option<EnchantmentEffects>),
        EnchantmentFile::split, EnchantmentFile::join,
        synced from parts as EnchantmentFile::join,
        received as EnchantmentFile => |file| file.split(),
        declared by EnchantmentFile::declaration();
    mcrs_minecraft_item::JukeboxSong => JukeboxSong, synced as Clone::clone, received as JukeboxSong => bare,
        declared by JukeboxSong::declaration();
    mcrs_minecraft_item::InstrumentValue => InstrumentValue, synced as Clone::clone, received as InstrumentValue => bare,
        declared by InstrumentValue::declaration();
    crate::test_types::TestEnvironment => TestEnvironment, synced as Clone::clone, received as TestEnvironment => bare,
        declared by TestEnvironment::declaration();
    crate::test_types::TestInstance => TestInstance, synced as Clone::clone, received as TestInstance => bare,
        declared by TestInstance::declaration();
    mcrs_minecraft_item::dialog::Dialog => Dialog, synced as Clone::clone, received as Dialog => bare,
        declared by Dialog::declaration();
    WorldClock => WorldClock, synced as Clone::clone, received as WorldClock => bare,
        declared by WorldClock::declaration();
    Timeline => Timeline, synced as |timeline| NetworkTimeline::from(timeline),
        received as NetworkTimeline => |timeline| bare(Timeline::from(timeline)),
        declared by Timeline::declaration(scope.clone());
    mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern => DecoratedPotPattern, synced as Clone::clone,
        received as DecoratedPotPattern => bare,
        declared by DecoratedPotPattern::declaration();
    @synced mcrs_minecraft_item::block_transformer::BlockTransformer => BlockTransformer,
        declared by BlockTransformer::declaration();
    @synced mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider => DirectBlockStateProvider,
        declared by DirectBlockStateProvider::declaration();
    @synced BlockSoundSet => BlockSoundSet,
        declared by BlockSoundSet::declaration();
    Dimension => DimensionEntry
        as (Dimension, ChunkGenerator),
        DimensionEntry::split, DimensionEntry::join,
        declared by DimensionEntry::declaration();
    mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList => MultiNoiseBiomeSourceParameterList,
        declared by MultiNoiseBiomeSourceParameterList::declaration();
    crate::worldgen::world_preset::WorldPreset => WorldPreset, declared by WorldPreset::declaration();
    crate::enchantment_provider::EnchantmentProvider => EnchantmentProvider,
        declared by EnchantmentProvider::declaration();
    crate::villager_trade::VillagerTrade => VillagerTrade, declared by VillagerTrade::declaration();
    crate::villager_trade::TradeSet => TradeSet, declared by TradeSet::declaration();
}

registry_table! {
    fn declare_reloadable_registries, fn reloadable_declarations, |_scope, stage|
    Recipe => Recipe, declared by Recipe::declaration();
    LootCondition => LootCondition, declared by LootCondition::declaration();
    LootItemFunction => LootItemFunction, declared by LootItemFunction::declaration();
    SlotSource => SlotSource, declared by SlotSource::declaration();
    LootTable => LootTableFile
        as (LootTable, LootTableBody), LootTableFile::split, LootTableFile::join,
        declared by LootTableFile::declaration();
    ContextIntProvider => IntExpression
        as (ContextIntProvider, IntExpression), IntExpression::split, IntExpression::join,
        declared by IntExpression::declaration();
    ContextFloatProvider => FloatExpression
        as (ContextFloatProvider, FloatExpression), FloatExpression::split, FloatExpression::join,
        declared by FloatExpression::declaration();
}

/// The registries a data pack reload reads, parsed over the world registries.
pub fn reloadable_registries(datapack_report: &[u8]) -> Result<WorldRegistries, LoadReport> {
    let mut reloadable = WorldRegistries::reloadable_from_datapack_report(datapack_report)
        .map_err(LoadReport::invalid)?;
    let mut undeclared = LoadReport::new();
    declare_reloadable_registries(&mut reloadable, &mut undeclared, RegistrySet::new);
    if undeclared.is_empty() {
        Ok(reloadable)
    } else {
        Err(undeclared)
    }
}

pub fn world_registries(datapack_report: &[u8]) -> Result<WorldRegistries, LoadReport> {
    let mut world =
        WorldRegistries::from_datapack_report(datapack_report).map_err(LoadReport::invalid)?;
    let mut undeclared = LoadReport::new();
    declare_world_registries(&mut world, &mut undeclared, RegistrySet::new);
    if undeclared.is_empty() {
        Ok(world)
    } else {
        Err(undeclared)
    }
}

fn declared<K, V>(
    world: &WorldRegistries,
    report: &mut LoadReport,
    declaration: &DataDeclaration<RegistryOf<V>>,
) -> bool
where
    K: mcrs_minecraft_core::Registered,
    V: Send + Sync + 'static,
{
    let registry = K::REGISTRY.location();
    if declaration.key().as_str() != registry.as_str() {
        report.invalid_report(format_args!(
            "{registry} is declared under the key {}",
            declaration.key()
        ));
        false
    } else if world
        .declared()
        .any(|declared| declared.as_str() == registry.as_str())
    {
        true
    } else {
        report.invalid_report(format_args!(
            "{registry} is not a {:?} registry of the data pack report",
            declaration.stage()
        ));
        false
    }
}

fn declare<K, V>(
    world: &mut WorldRegistries,
    report: &mut LoadReport,
    declaration: DataDeclaration<RegistryOf<V>>,
) where
    K: mcrs_minecraft_core::Registered,
    V: Serialize + Send + Sync + 'static,
{
    if declared::<K, V>(world, report, &declaration) {
        let registry = K::REGISTRY.location();
        world.parse::<V>(registry);
        if declaration.is_non_empty() {
            world.non_empty(registry);
        }
    }
}

/// The registries a server holding `server` sends, in table order. A vanilla entry
/// goes without data unless `sends_data` asks for it, as it does to a client that
/// knows the vanilla pack.
pub fn registries_as_sent(
    server: &RegistrySet,
    sends_data: impl Fn(&str, usize) -> bool,
) -> Vec<mcrs_minecraft_registry::NetworkRegistry> {
    server
        .synced()
        .map(|(table, column)| {
            let registry = table.registry();
            mcrs_minecraft_registry::NetworkRegistry {
                registry: registry.clone(),
                entries: table
                    .names()
                    .iter()
                    .zip(column)
                    .enumerate()
                    .map(
                        |(id, (name, network))| mcrs_minecraft_registry::NetworkEntry {
                            name: name.clone(),
                            data: sends_data(registry.as_str(), id).then(|| network.0.clone()),
                        },
                    )
                    .collect(),
            }
        })
        .collect()
}

/// Every vanilla entry of `server` without data and every other entry with it.
pub fn registries_as_sent_to_a_client_that_knows_vanilla(
    server: &RegistrySet,
) -> Vec<mcrs_minecraft_registry::NetworkRegistry> {
    registries_as_sent(server, |registry, id| {
        server.pack_of(registry, id) != Some(mcrs_minecraft_registry::VANILLA_PACK)
    })
}

/// The tags a server holding `server` sends for the static registries and the synced ones.
pub fn tags_as_sent(
    statics: &RegistrySet,
    server: &RegistrySet,
) -> Vec<mcrs_minecraft_registry::NetworkTags> {
    let synced: Vec<_> = server
        .synced()
        .map(|(table, _)| table.registry().as_str())
        .collect();
    server
        .tables()
        .map(|table| table.registry())
        .filter(|registry| {
            statics.table(registry.as_str()).is_some() || synced.contains(&registry.as_str())
        })
        .filter_map(|registry| server.tag_table(registry.as_str()))
        .filter(|table| !table.is_empty())
        .map(|table| mcrs_minecraft_registry::NetworkTags {
            registry: table.registry().clone(),
            tags: table
                .names()
                .iter()
                .enumerate()
                .map(|(tag, name)| {
                    let members = table.members(tag).iter().map(|&m| i32::from(m)).collect();
                    (name.clone(), members)
                })
                .collect(),
        })
        .collect()
}

pub fn static_registries() -> Result<RegistrySet, LoadReport> {
    RegistrySet::from_locations(mcrs_minecraft_registry_catalog::STATIC_REGISTRIES)
        .and_then(|set| {
            set.with_types(
                mcrs_minecraft_registry_catalog::bindings().chain(crate::keys::bindings()),
            )
        })
        .map_err(LoadReport::invalid)
}

pub fn refuse<E: std::fmt::Display>(error: &E) -> ! {
    tracing::error!("the registries are unusable:\n{error}");
    std::process::exit(1)
}
