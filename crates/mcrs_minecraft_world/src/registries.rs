use crate::chat_type::ChatType;
use crate::data_pack::walk_files;
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
use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::{AssetSourceId, ErasedAssetReader};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use bevy_ecs::world::World;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::{PACKS_ROOT, VANILLA_PACK, layered_file_source, pack_names};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList;
use mcrs_minecraft_biome_file::{BiomeFile, BiomeGenerationSettings, NetworkBiome};
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
use mcrs_minecraft_environment::world_clock::{ClockTimeMarkers, WorldClock, check_time_markers};
use mcrs_minecraft_item::block_transformer::BlockTransformer;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern;
use mcrs_minecraft_item::dialog::Dialog;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::loot::{ContextFloatProvider, ContextIntProvider, LootTable};
use mcrs_minecraft_item::recipe::Recipe;
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, Items, JukeboxSong, PaintingVariantValue, TrimMaterial,
    TrimPattern,
};
use mcrs_minecraft_loot::number::{FloatExpression, IntExpression};
use mcrs_minecraft_loot::{
    LootCondition, LootItemFunction, LootTableBody, LootTableFile, SlotSource,
};
use mcrs_minecraft_registry::shared::share;
use mcrs_minecraft_registry::{
    Entries, LoadReport, Pack, PackFile, Parts, Registry, RegistrySet, WorldRegistries,
};
use mcrs_minecraft_worldgen_structure::Structure;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::LazyLock;

macro_rules! world_registry_table {
    ($(
        $key:ty => $value:ty $([$non_empty:ident])?
            $(as $parts:ty, $split:expr, $join:expr $(, synced from parts as $part_project:expr)?)?
            $(, synced as $project:expr)?;
    )*) => {
        fn declare_world_registries(world: &mut WorldRegistries, report: &mut LoadReport) {
            $(
                parse::<$key, $value>(world, report);
                let registry = <$key as mcrs_minecraft_registry::Registered>::REGISTRY.location();
                if world.parses(registry.as_static_str()) {
                    $(world.split::<$value, $parts>(registry, $split, $join);)?
                    $(world.$non_empty(registry);)?
                    $($(world.sync_parts::<$parts, _>(registry, $part_project);)?)?
                    $(world.sync_value::<$value, _>(registry, $project);)?
                }
            )*
        }
    };
}

world_registry_table! {
    Biome => BiomeFile
        as (Biome, EnvironmentAttributeMap, BiomeGenerationSettings),
        BiomeFile::split, BiomeFile::join,
        synced from parts as |parts| NetworkBiome::from(parts);
    crate::chat_type::ChatType => ChatType, synced as Clone::clone;
    mcrs_minecraft_item::TrimPattern => TrimPattern, synced as Clone::clone;
    mcrs_minecraft_item::TrimMaterial => TrimMaterial, synced as Clone::clone;
    WolfVariant => WolfVariantFile [non_empty]
        as (WolfVariant, Vec<SpawnSelector>),
        WolfVariantFile::split, WolfVariantFile::join,
        synced from parts as |(variant, _)| variant.clone();
    WolfSoundVariant => WolfSoundVariant [non_empty], synced as Clone::clone;
    PigVariant => PigVariantFile [non_empty]
        as (PigVariant, Vec<SpawnSelector>),
        PigVariantFile::split, PigVariantFile::join,
        synced from parts as |(variant, _)| variant.clone();
    PigSoundVariant => PigSoundVariant [non_empty], synced as Clone::clone;
    FrogVariant => FrogVariantFile [non_empty]
        as (FrogVariant, Vec<SpawnSelector>),
        FrogVariantFile::split, FrogVariantFile::join,
        synced from parts as |(variant, _)| variant.clone();
    CatVariant => CatVariantFile [non_empty]
        as (CatVariant, Vec<SpawnSelector>),
        CatVariantFile::split, CatVariantFile::join,
        synced from parts as |(variant, _)| variant.clone();
    CatSoundVariant => CatSoundVariant [non_empty], synced as Clone::clone;
    CowSoundVariant => CowSoundVariant [non_empty], synced as Clone::clone;
    CowVariant => CowVariantFile [non_empty]
        as (CowVariant, Vec<SpawnSelector>),
        CowVariantFile::split, CowVariantFile::join,
        synced from parts as |(variant, _)| variant.clone();
    ChickenSoundVariant => ChickenSoundVariant [non_empty], synced as Clone::clone;
    ChickenVariant => ChickenVariantFile [non_empty]
        as (ChickenVariant, Vec<SpawnSelector>),
        ChickenVariantFile::split, ChickenVariantFile::join,
        synced from parts as |(variant, _)| variant.clone();
    ZombieNautilusVariant => ZombieNautilusVariantFile [non_empty]
        as (ZombieNautilusVariant, Vec<SpawnSelector>),
        ZombieNautilusVariantFile::split, ZombieNautilusVariantFile::join,
        synced from parts as |(variant, _)| variant.clone();
    mcrs_minecraft_item::PaintingVariantValue => PaintingVariantValue [non_empty], synced as Clone::clone;
    crate::sulfur_cube_archetype::SulfurCubeArchetype => SulfurCubeArchetype, synced as Clone::clone;
    DimensionType => DimensionTypeFile
        as (DimensionType, DimensionTypeEnvironment),
        DimensionTypeFile::split, DimensionTypeFile::join,
        synced from parts as |parts| DimensionTypeFile::join(parts).synced();
    mcrs_minecraft_item::damage_type::DamageType => DamageType, synced as Clone::clone;
    mcrs_minecraft_item::BannerPattern => BannerPattern, synced as Clone::clone;
    mcrs_minecraft_item::enchantment::EnchantmentData => EnchantmentFile
        as (EnchantmentData, Option<EnchantmentEffects>),
        EnchantmentFile::split, EnchantmentFile::join,
        synced from parts as EnchantmentFile::join;
    mcrs_minecraft_item::JukeboxSong => JukeboxSong, synced as Clone::clone;
    mcrs_minecraft_item::InstrumentValue => InstrumentValue, synced as Clone::clone;
    crate::test_types::TestEnvironment => TestEnvironment, synced as Clone::clone;
    crate::test_types::TestInstance => TestInstance, synced as Clone::clone;
    mcrs_minecraft_item::dialog::Dialog => Dialog, synced as Clone::clone;
    WorldClock => WorldClock, synced as Clone::clone;
    Timeline => Timeline, synced as |timeline| NetworkTimeline::from(timeline);
    mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern => DecoratedPotPattern, synced as Clone::clone;
    mcrs_minecraft_item::block_transformer::BlockTransformer => BlockTransformer, synced as Clone::clone;
    mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider => DirectBlockStateProvider, synced as Clone::clone;
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

pub fn read_packs(
    asset_server: &AssetServer,
    registries: &WorldRegistries,
    statics: &RegistrySet,
) -> Vec<Pack> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing");
    let reader = source.reader();
    bevy_tasks::block_on(async {
        let mut packs =
            vec![read_pack(reader, VANILLA_PACK, Path::new(""), registries, statics).await];
        for name in pack_names(reader).await {
            let root = Path::new(PACKS_ROOT).join(&name);
            packs.push(read_pack(reader, &name, &root, registries, statics).await);
        }
        packs
    })
}

async fn read_pack(
    reader: &dyn ErasedAssetReader,
    name: &str,
    root: &Path,
    registries: &WorldRegistries,
    statics: &RegistrySet,
) -> Pack {
    let vanilla = root.as_os_str().is_empty();
    let mut namespaces = Vec::new();
    if let Ok(mut listing) = reader.read_directory(root).await {
        while let Some(entry) = listing.next().await {
            if let Some(namespace) = entry.file_name().and_then(|name| name.to_str())
                && !(vanilla && namespace == "mcrs")
            {
                namespaces.push(namespace.to_owned());
            }
        }
    }

    let tag_directories: BTreeSet<&str> = statics
        .tables()
        .map(|table| table.registry().path())
        .chain(registries.declared().map(|registry| registry.path()))
        .collect();

    let mut found: BTreeMap<String, bool> = BTreeMap::new();
    for namespace in &namespaces {
        for registry in registries.declared() {
            let reads_bytes = registries.parses(registry.as_str());
            let directory = root.join(namespace).join(registry.path());
            for path in walk_files(reader, directory).await {
                if let Some(path) = relative_to(root, &path) {
                    *found.entry(path).or_default() |= reads_bytes;
                }
            }
            if vanilla {
                let directory = format!("{namespace}/{}", registry.path());
                for path in mcrs_minecraft_worldgen_builtin::paths(&directory) {
                    *found.entry(path).or_default() |= reads_bytes;
                }
            }
        }
        for directory in &tag_directories {
            let directory = root.join(namespace).join("tags").join(directory);
            for path in walk_files(reader, directory).await {
                if let Some(path) = relative_to(root, &path) {
                    found.insert(path, true);
                }
            }
        }
    }

    let mut files = Vec::with_capacity(found.len());
    for (path, reads_bytes) in found {
        let bytes = if reads_bytes {
            match read_whole(reader, &root.join(&path)).await {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    tracing::warn!(path, %error, "a registry file does not read");
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
        built: if vanilla
            && registries.parses(mcrs_minecraft_biome::keys::BIOME.location().as_static_str())
        {
            vec![mcrs_minecraft_worldgen_builtin::built_biomes()]
        } else {
            Vec::new()
        },
    }
}

fn relative_to(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .map(str::to_owned)
}

pub fn load_registries(
    asset_server: &AssetServer,
    statics: RegistrySet,
) -> Result<RegistrySet, LoadReport> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing");
    let path = Path::new("mcrs/reports/datapack.json");
    let bytes = bevy_tasks::block_on(read_whole(source.reader(), path))
        .map_err(|error| LoadReport::invalid(format_args!("{}: {error}", path.display())))?;
    let world = world_registries(&bytes)?;
    let packs = read_packs(asset_server, &world, &statics);
    let loaded = world.load(&statics, &packs)?;
    let reloadable = reloadable_registries(&bytes)?;
    let packs = read_packs(asset_server, &reloadable, &loaded);
    reloadable.load(&loaded, &packs)
}

pub fn test_registries() -> &'static RegistrySet {
    static SET: LazyLock<RegistrySet> = LazyLock::new(|| {
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            layered_file_source(
                &AssetPlugin::default().file_path,
                mcrs_minecraft_worldgen_builtin::asset,
            ),
        );
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            },
        ));
        let asset_server = app.world().resource::<AssetServer>().clone();
        let statics = static_registries().unwrap_or_else(|report| refuse(&report));
        load_registries(&asset_server, statics).unwrap_or_else(|report| refuse(&report))
    });
    &SET
}

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
