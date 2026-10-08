use crate::chat_type::ChatType;
#[cfg(feature = "bevy")]
use crate::data_pack::walk_files;
use crate::dimension::DimensionEntry;
use crate::enchantment_provider::EnchantmentProvider;
#[cfg(feature = "bevy")]
use crate::packs::PackScan;
use crate::sulfur_cube_archetype::SulfurCubeArchetype;
use crate::test_types::{TestEnvironment, TestInstance};
use crate::variant::{
    CatVariantFile, ChickenVariantFile, CowVariantFile, FrogVariantFile, PigVariantFile,
    SpawnSelector, WolfVariantFile, ZombieNautilusVariantFile,
};
use crate::villager_trade::{TradeSet, VillagerTrade};
use crate::worldgen::chunk_generator::ChunkGenerator;
use crate::worldgen::world_preset::WorldPreset;
#[cfg(feature = "bevy")]
use bevy_asset::AssetServer;
#[cfg(feature = "bevy")]
use bevy_asset::io::{AssetSourceId, ErasedAssetReader};
#[cfg(feature = "bevy")]
use bevy_ecs::world::World;
#[cfg(feature = "bevy")]
use bevy_tasks::futures_lite::StreamExt;
#[cfg(feature = "bevy")]
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList;
use mcrs_minecraft_biome_file::{
    BiomeFile, BiomeGenerationSettings, NetworkBiome, check_feature_domains,
};
#[cfg(feature = "bevy")]
use mcrs_minecraft_block::definition::Blocks;
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
#[cfg(feature = "bevy")]
use mcrs_minecraft_environment::world_clock::ClockTimeMarkers;
use mcrs_minecraft_environment::world_clock::{WorldClock, check_time_markers};
#[cfg(feature = "bevy")]
use mcrs_minecraft_item::Items;
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
#[cfg(feature = "bevy")]
use mcrs_minecraft_registry::shared::share;
#[cfg(feature = "bevy")]
use mcrs_minecraft_registry::{Entries, Pack, PackFile, Registry};
use mcrs_minecraft_registry::{LoadReport, Parts, RegistrySet, WorldRegistries};
#[cfg(feature = "bevy")]
use mcrs_minecraft_registry::{PACKS_ROOT, VANILLA_PACK};
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_surface::proto::{
    MaterialConditionHolder, MaterialRule, MaterialRuleHolder,
};
#[cfg(feature = "bevy")]
use mcrs_minecraft_worldgen_structure::Structure;
use serde::Serialize;
use serde::de::DeserializeOwned;
#[cfg(feature = "bevy")]
use std::path::Path;
use std::sync::LazyLock;

macro_rules! world_registry_table {
    ($(
        $key:ty => $value:ty $([$non_empty:ident])?
            $(as $parts:ty, $split:expr, $join:expr
                $(, synced from parts as $part_project:expr, received as $part_net:ty => $part_receive:expr)?)?
            $(, synced as $project:expr, received as $net:ty => $receive:expr)?;
    )*) => {
        fn declare_world_registries(world: &mut WorldRegistries, report: &mut LoadReport) {
            $(
                parse::<$key, $value>(world, report);
                let registry = <$key as mcrs_minecraft_registry::Registered>::REGISTRY.location();
                if world.parses(registry.as_static_str()) {
                    $(world.split::<$value, $parts>(registry, $split, $join);)?
                    $(world.$non_empty(registry);)?
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
    };
}

fn bare<T>(value: T) -> (T,) {
    (value,)
}

world_registry_table! {
    Biome => BiomeFile
        as (Biome, EnvironmentAttributeMap, BiomeGenerationSettings),
        BiomeFile::split, BiomeFile::join,
        synced from parts as |parts| NetworkBiome::from(parts),
        received as NetworkBiome => NetworkBiome::into_parts;
    PlacedFeature => PlacedFeature;
    NoiseParam => NoiseParam;
    DensityFunctionHolder => DensityFunctionHolder;
    NoiseGeneratorSettings => NoiseGeneratorSettings;
    MaterialRule => MaterialRuleHolder;
    MaterialConditionHolder => MaterialConditionHolder;
    crate::chat_type::ChatType => ChatType, synced as Clone::clone, received as ChatType => bare;
    mcrs_minecraft_item::TrimPattern => TrimPattern, synced as Clone::clone, received as TrimPattern => bare;
    mcrs_minecraft_item::TrimMaterial => TrimMaterial, synced as Clone::clone, received as TrimMaterial => bare;
    WolfVariant => WolfVariantFile [non_empty]
        as (WolfVariant, Vec<SpawnSelector>),
        WolfVariantFile::split, WolfVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as WolfVariant => bare;
    WolfSoundVariant => WolfSoundVariant [non_empty], synced as Clone::clone, received as WolfSoundVariant => bare;
    PigVariant => PigVariantFile [non_empty]
        as (PigVariant, Vec<SpawnSelector>),
        PigVariantFile::split, PigVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as PigVariant => bare;
    PigSoundVariant => PigSoundVariant [non_empty], synced as Clone::clone, received as PigSoundVariant => bare;
    FrogVariant => FrogVariantFile [non_empty]
        as (FrogVariant, Vec<SpawnSelector>),
        FrogVariantFile::split, FrogVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as FrogVariant => bare;
    CatVariant => CatVariantFile [non_empty]
        as (CatVariant, Vec<SpawnSelector>),
        CatVariantFile::split, CatVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as CatVariant => bare;
    CatSoundVariant => CatSoundVariant [non_empty], synced as Clone::clone, received as CatSoundVariant => bare;
    CowSoundVariant => CowSoundVariant [non_empty], synced as Clone::clone, received as CowSoundVariant => bare;
    CowVariant => CowVariantFile [non_empty]
        as (CowVariant, Vec<SpawnSelector>),
        CowVariantFile::split, CowVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as CowVariant => bare;
    ChickenSoundVariant => ChickenSoundVariant [non_empty], synced as Clone::clone, received as ChickenSoundVariant => bare;
    ChickenVariant => ChickenVariantFile [non_empty]
        as (ChickenVariant, Vec<SpawnSelector>),
        ChickenVariantFile::split, ChickenVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as ChickenVariant => bare;
    ZombieNautilusVariant => ZombieNautilusVariantFile [non_empty]
        as (ZombieNautilusVariant, Vec<SpawnSelector>),
        ZombieNautilusVariantFile::split, ZombieNautilusVariantFile::join,
        synced from parts as |(variant, _)| variant.clone(),
        received as ZombieNautilusVariant => bare;
    mcrs_minecraft_item::PaintingVariantValue => PaintingVariantValue [non_empty], synced as Clone::clone, received as PaintingVariantValue => bare;
    crate::sulfur_cube_archetype::SulfurCubeArchetype => SulfurCubeArchetype, synced as Clone::clone, received as SulfurCubeArchetype => bare;
    DimensionType => DimensionTypeFile
        as (DimensionType, DimensionTypeEnvironment),
        DimensionTypeFile::split, DimensionTypeFile::join,
        synced from parts as |parts| DimensionTypeFile::join(parts).synced(),
        received as DimensionTypeFile => |file| file.split();
    mcrs_minecraft_item::damage_type::DamageType => DamageType, synced as Clone::clone, received as DamageType => bare;
    mcrs_minecraft_item::BannerPattern => BannerPattern, synced as Clone::clone, received as BannerPattern => bare;
    mcrs_minecraft_item::enchantment::EnchantmentData => EnchantmentFile
        as (EnchantmentData, Option<EnchantmentEffects>),
        EnchantmentFile::split, EnchantmentFile::join,
        synced from parts as EnchantmentFile::join,
        received as EnchantmentFile => |file| file.split();
    mcrs_minecraft_item::JukeboxSong => JukeboxSong, synced as Clone::clone, received as JukeboxSong => bare;
    mcrs_minecraft_item::InstrumentValue => InstrumentValue, synced as Clone::clone, received as InstrumentValue => bare;
    crate::test_types::TestEnvironment => TestEnvironment, synced as Clone::clone, received as TestEnvironment => bare;
    crate::test_types::TestInstance => TestInstance, synced as Clone::clone, received as TestInstance => bare;
    mcrs_minecraft_item::dialog::Dialog => Dialog, synced as Clone::clone, received as Dialog => bare;
    WorldClock => WorldClock, synced as Clone::clone, received as WorldClock => bare;
    Timeline => Timeline, synced as |timeline| NetworkTimeline::from(timeline),
        received as NetworkTimeline => |timeline| bare(Timeline::from(timeline));
    mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern => DecoratedPotPattern, synced as Clone::clone, received as DecoratedPotPattern => bare;
    mcrs_minecraft_item::block_transformer::BlockTransformer => BlockTransformer, synced as Clone::clone, received as BlockTransformer => bare;
    mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider => DirectBlockStateProvider, synced as Clone::clone, received as DirectBlockStateProvider => bare;
    Dimension => DimensionEntry
        as (Dimension, ChunkGenerator),
        DimensionEntry::split, DimensionEntry::join;
    mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList => MultiNoiseBiomeSourceParameterList;
    crate::worldgen::world_preset::WorldPreset => WorldPreset;
    crate::enchantment_provider::EnchantmentProvider => EnchantmentProvider;
    crate::villager_trade::VillagerTrade => VillagerTrade;
    crate::villager_trade::TradeSet => TradeSet;
}

fn parse_reloadable_registries(reloadable: &mut WorldRegistries, report: &mut LoadReport) {
    parse::<Recipe, Recipe>(reloadable, report);
    parse::<LootCondition, LootCondition>(reloadable, report);
    parse::<LootItemFunction, LootItemFunction>(reloadable, report);
    parse::<SlotSource, SlotSource>(reloadable, report);
    parse_split::<LootTable, LootTableFile, (LootTable, LootTableBody)>(
        reloadable,
        report,
        LootTableFile::split,
        LootTableFile::join,
    );
    parse_split::<ContextIntProvider, IntExpression, (ContextIntProvider, IntExpression)>(
        reloadable,
        report,
        IntExpression::split,
        IntExpression::join,
    );
    parse_split::<ContextFloatProvider, FloatExpression, (ContextFloatProvider, FloatExpression)>(
        reloadable,
        report,
        FloatExpression::split,
        FloatExpression::join,
    );
}

/// The registries a data pack reload reads, parsed over the world registries.
pub fn reloadable_registries(datapack_report: &[u8]) -> Result<WorldRegistries, LoadReport> {
    let mut reloadable = WorldRegistries::reloadable_from_datapack_report(datapack_report)
        .map_err(LoadReport::invalid)?;
    let mut undeclared = LoadReport::new();
    parse_reloadable_registries(&mut reloadable, &mut undeclared);
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
    declare_world_registries(&mut world, &mut undeclared);
    if world.parses(
        mcrs_minecraft_environment::keys::TIMELINE
            .location()
            .as_static_str(),
    ) {
        world.validate::<Timeline>(
            mcrs_minecraft_environment::keys::TIMELINE.location(),
            check_time_markers,
        );
    }
    if world.parses(mcrs_minecraft_biome::keys::BIOME.location().as_static_str()) {
        world.validate::<BiomeGenerationSettings>(
            mcrs_minecraft_biome::keys::BIOME.location(),
            check_feature_domains,
        );
    }
    if undeclared.is_empty() {
        Ok(world)
    } else {
        Err(undeclared)
    }
}

fn parse<K, T>(world: &mut WorldRegistries, report: &mut LoadReport)
where
    K: mcrs_minecraft_registry::Registered,
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    let registry = K::REGISTRY.location();
    if world
        .declared()
        .any(|declared| declared.as_str() == registry.as_str())
    {
        world.parse::<T>(registry);
    } else {
        report.invalid_report(format_args!(
            "{registry} is not a world registry of the data pack report"
        ));
    }
}

fn parse_split<K, T, P>(
    world: &mut WorldRegistries,
    report: &mut LoadReport,
    split: fn(&T) -> P,
    join: for<'a> fn(P::Refs<'a>) -> T,
) where
    K: mcrs_minecraft_registry::Registered,
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    P: Parts,
{
    parse::<K, T>(world, report);
    let registry = K::REGISTRY.location();
    if world.parses(registry.as_static_str()) {
        world.split::<T, P>(registry, split, join);
    }
}

#[cfg(feature = "bevy")]
pub fn read_packs(
    asset_server: &AssetServer,
    registries: &WorldRegistries,
    statics: &RegistrySet,
) -> Result<Vec<Pack>, LoadReport> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing");
    let reader = source.reader();
    bevy_tasks::block_on(async {
        let mut report = LoadReport::new();
        let mut packs = vec![
            read_pack(
                reader,
                VANILLA_PACK,
                Path::new(""),
                registries,
                statics,
                &mut report,
            )
            .await,
        ];
        for name in mcrs_minecraft_assets::packs::pack_names(reader).await {
            let root = Path::new(PACKS_ROOT).join(&name);
            packs.push(read_pack(reader, &name, &root, registries, statics, &mut report).await);
        }
        if report.is_empty() {
            Ok(packs)
        } else {
            Err(report)
        }
    })
}

#[cfg(feature = "bevy")]
async fn read_pack(
    reader: &dyn ErasedAssetReader,
    name: &str,
    root: &Path,
    registries: &WorldRegistries,
    statics: &RegistrySet,
    report: &mut LoadReport,
) -> Pack {
    let mut scan = PackScan::new(name, registries, statics);
    let mut namespaces = Vec::new();
    if let Ok(mut listing) = reader.read_directory(root).await {
        while let Some(entry) = listing.next().await {
            if let Some(namespace) = entry.file_name().and_then(|name| name.to_str())
                && scan.keeps_namespace(namespace)
            {
                namespaces.push(namespace.to_owned());
            }
        }
    }
    for namespace in &namespaces {
        for directory in scan.directories(namespace) {
            for file in walk_files(reader, root.join(&directory.path)).await {
                scan.record(&directory, root, &file);
            }
        }
    }

    let (listing, built) = scan.finish();
    let mut files = Vec::with_capacity(listing.len());
    for (path, reads_bytes) in listing {
        let bytes = if reads_bytes {
            match read_whole(reader, &root.join(&path)).await {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    report.invalid_report(format_args!("{}: {error}", root.join(&path).display()));
                    None
                }
            }
        } else {
            None
        };
        files.push(PackFile { path, bytes });
    }
    Pack {
        name: name.to_owned(),
        files,
        built,
    }
}

#[cfg(feature = "bevy")]
pub fn load_registries(
    asset_server: &AssetServer,
    statics: RegistrySet,
) -> Result<RegistrySet, LoadReport> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing");
    let path = Path::new(crate::packs::DATAPACK_REPORT);
    let bytes = bevy_tasks::block_on(read_whole(source.reader(), path))
        .map_err(|error| LoadReport::invalid(format_args!("{}: {error}", path.display())))?;
    crate::packs::load_registry_set(statics, &bytes, |registries, statics| {
        read_packs(asset_server, registries, statics)
    })
}

pub fn test_registries() -> &'static RegistrySet {
    static SET: LazyLock<RegistrySet> = LazyLock::new(|| {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        crate::packs::load_from_directory(&assets).unwrap_or_else(|report| refuse(&report))
    });
    &SET
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

#[cfg(feature = "bevy")]
pub fn insert_registry_resources(world: &mut World, registries: &RegistrySet) {
    world.insert_resource(
        registries
            .registry::<Biome>()
            .expect("the data pack loader parses minecraft:worldgen/biome"),
    );
    world.insert_resource(
        registries
            .registry::<Structure>()
            .expect("the data pack declares minecraft:worldgen/structure"),
    );
    world.insert_resource(
        registries
            .registry::<EnchantmentData>()
            .expect("the data pack loader parses minecraft:enchantment"),
    );
    world.insert_resource(
        registries
            .entries::<EnchantmentData, EnchantmentData>()
            .expect("the data pack loader parses minecraft:enchantment"),
    );
    world.insert_resource(
        registries
            .entries::<EnchantmentData, Option<EnchantmentEffects>>()
            .expect("the data pack loader splits minecraft:enchantment"),
    );
    let clocks = registries
        .registry::<WorldClock>()
        .expect("the data pack loader parses minecraft:world_clock");
    let timelines = registries
        .column::<Timeline>(
            mcrs_minecraft_environment::keys::TIMELINE
                .location()
                .as_static_str(),
        )
        .expect("the data pack loader parses minecraft:timeline");
    world.insert_resource(
        ClockTimeMarkers::derive(timelines, &clocks)
            .expect("the load refused a time marker defined twice for one clock"),
    );
    world.insert_resource(
        registries
            .registry::<Timeline>()
            .expect("the data pack loader parses minecraft:timeline"),
    );
    world.insert_resource(
        registries
            .registry::<mcrs_minecraft_entity::keys::EntityType>()
            .expect("the registries report holds minecraft:entity_type"),
    );
    world.insert_resource(registries.clone());
}

#[cfg(feature = "bevy")]
pub fn share_registries(world: &mut World) {
    share::<RegistrySet>(world);
    share::<Blocks>(world);
    share::<Items>(world);
    share::<Registry<EnchantmentData>>(world);
    share::<Entries<EnchantmentData, EnchantmentData>>(world);
    share::<Entries<EnchantmentData, Option<EnchantmentEffects>>>(world);
    share::<Registry<Biome>>(world);
    share::<Registry<Structure>>(world);
    share::<Registry<Timeline>>(world);
    share::<Registry<mcrs_minecraft_entity::keys::EntityType>>(world);
    share::<ClockTimeMarkers>(world);
    share::<mcrs_minecraft_worldgen::tables::WorldgenTables>(world);
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

pub fn refuse(report: &LoadReport) -> ! {
    tracing::error!("the registries are unusable:\n{report}");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_constants_stand_at_the_entry_they_name() {
        use mcrs_minecraft_entity::keys::EntityType;
        let entity_types = [
            (EntityType::Allay, "allay"),
            (EntityType::Cat, "cat"),
            (EntityType::ChestMinecart, "chest_minecart"),
            (EntityType::Chicken, "chicken"),
            (EntityType::Drowned, "drowned"),
            (EntityType::ElderGuardian, "elder_guardian"),
            (EntityType::Evoker, "evoker"),
            (EntityType::Item, "item"),
            (EntityType::ItemFrame, "item_frame"),
            (EntityType::Player, "player"),
            (EntityType::Shulker, "shulker"),
            (EntityType::Tnt, "tnt"),
            (EntityType::Villager, "villager"),
            (EntityType::Vindicator, "vindicator"),
            (EntityType::Witch, "witch"),
            (EntityType::ZombieNautilus, "zombie_nautilus"),
            (EntityType::ZombieVillager, "zombie_villager"),
        ];
        for (key, name) in entity_types {
            assert_eq!(key.as_static_str(), format!("minecraft:{name}"));
            assert_eq!(EntityType::ENTRIES[key.id().index()], key.location());
        }
        assert_eq!(
            mcrs_minecraft_block::keys::Block::Tnt.as_static_str(),
            "minecraft:tnt"
        );
        assert_eq!(
            mcrs_minecraft_block::keys::Block::ENTRIES
                [mcrs_minecraft_block::keys::Block::Tnt.id().index()],
            mcrs_minecraft_block::keys::Block::Tnt.location()
        );
        assert_eq!(
            mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str(),
            "minecraft:max_health"
        );
    }
}
