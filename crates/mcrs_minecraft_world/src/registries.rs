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
use mcrs_minecraft_assets::{PackSource, RegistryAccess, RegistryEntry, SyncedRegistry};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::parameter_list::{
    MultiNoiseBiomeSourceParameterList, check_parameter_list_biomes,
};
use mcrs_minecraft_biome_file::{BiomeFile, BiomeGenerationSettings, NetworkBiome};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_dimension_environment::dimension_type::{
    DimensionTypeEnvironment, DimensionTypeFile, NetworkDimensionType,
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
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, Items, JukeboxSong, PaintingVariantValue, TrimMaterial,
    TrimPattern,
};
use mcrs_minecraft_keys as keys;
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
    ($($key:ty => $value:ty $([$non_empty:ident])? $(, synced as $project:expr)?;)*) => {
        fn parse_world_registries(world: &mut WorldRegistries, report: &mut LoadReport) {
            $(
                parse::<$key, $value>(world, report);
                $(
                    if world.parses(<$key as mcrs_minecraft_registry::Registered>::REGISTRY.location().as_static_str()) {
                        world.$non_empty(<$key as mcrs_minecraft_registry::Registered>::REGISTRY.location());
                    }
                )?
            )*
        }

        pub fn register_world_registries(access: &mut RegistryAccess, set: &RegistrySet) {
            $($(
                register_loaded::<$value, _>(
                    access,
                    set,
                    <$key as mcrs_minecraft_registry::Registered>::REGISTRY.location().as_static_str(),
                    $project,
                );
            )?)*
        }
    };
}

macro_rules! split_registry_table {
    ($($key:ty => $file:ty as $parts:ty $([$non_empty:ident])?, $split:expr, $join:expr $(, synced as $project:expr)?;)*) => {
        fn parse_split_registries(world: &mut WorldRegistries, report: &mut LoadReport) {
            $(
                parse::<$key, $file>(world, report);
                let registry = <$key as mcrs_minecraft_registry::Registered>::REGISTRY.location();
                if world.parses(registry.as_static_str()) {
                    world.split::<$file, $parts>(registry, $split, $join);
                    $(world.$non_empty(registry);)?
                }
            )*
        }

        pub fn register_split_registries(access: &mut RegistryAccess, set: &RegistrySet) {
            $($(
                register_joined::<$file, $parts, _>(
                    access,
                    set,
                    <$key as mcrs_minecraft_registry::Registered>::REGISTRY.location().as_static_str(),
                    $join,
                    $project,
                );
            )?)*
        }
    };
}

split_registry_table! {
    WolfVariant => WolfVariantFile as (WolfVariant, Vec<SpawnSelector>) [non_empty],
        WolfVariantFile::split, WolfVariantFile::join, synced as WolfVariantFile::synced;
    PigVariant => PigVariantFile as (PigVariant, Vec<SpawnSelector>) [non_empty],
        PigVariantFile::split, PigVariantFile::join, synced as PigVariantFile::synced;
    CowVariant => CowVariantFile as (CowVariant, Vec<SpawnSelector>) [non_empty],
        CowVariantFile::split, CowVariantFile::join, synced as CowVariantFile::synced;
    ChickenVariant => ChickenVariantFile as (ChickenVariant, Vec<SpawnSelector>) [non_empty],
        ChickenVariantFile::split, ChickenVariantFile::join, synced as ChickenVariantFile::synced;
    CatVariant => CatVariantFile as (CatVariant, Vec<SpawnSelector>) [non_empty],
        CatVariantFile::split, CatVariantFile::join, synced as CatVariantFile::synced;
    FrogVariant => FrogVariantFile as (FrogVariant, Vec<SpawnSelector>) [non_empty],
        FrogVariantFile::split, FrogVariantFile::join, synced as FrogVariantFile::synced;
    ZombieNautilusVariant => ZombieNautilusVariantFile as (ZombieNautilusVariant, Vec<SpawnSelector>) [non_empty],
        ZombieNautilusVariantFile::split, ZombieNautilusVariantFile::join, synced as ZombieNautilusVariantFile::synced;
    mcrs_minecraft_item::enchantment::EnchantmentData => EnchantmentFile as (EnchantmentData, Option<EnchantmentEffects>),
        EnchantmentFile::split, EnchantmentFile::join, synced as Clone::clone;
    Biome => BiomeFile as (Biome, EnvironmentAttributeMap, BiomeGenerationSettings),
        BiomeFile::split, BiomeFile::join, synced as |biome| NetworkBiome::from(biome);
    DimensionType => DimensionTypeFile as (DimensionType, DimensionTypeEnvironment),
        DimensionTypeFile::split, DimensionTypeFile::join,
        synced as |d| NetworkDimensionType::from(d);
    Dimension => DimensionEntry as (Dimension, ChunkGenerator),
        DimensionEntry::split, DimensionEntry::join;
}

world_registry_table! {
    mcrs_minecraft_item::BannerPattern => BannerPattern, synced as Clone::clone;
    mcrs_minecraft_item::InstrumentValue => InstrumentValue, synced as Clone::clone;
    mcrs_minecraft_item::JukeboxSong => JukeboxSong, synced as Clone::clone;
    mcrs_minecraft_item::PaintingVariantValue => PaintingVariantValue [non_empty], synced as Clone::clone;
    mcrs_minecraft_item::TrimMaterial => TrimMaterial, synced as Clone::clone;
    mcrs_minecraft_item::TrimPattern => TrimPattern, synced as Clone::clone;
    crate::chat_type::ChatType => ChatType, synced as Clone::clone;
    crate::test_types::TestEnvironment => TestEnvironment, synced as Clone::clone;
    crate::test_types::TestInstance => TestInstance, synced as Clone::clone;
    mcrs_minecraft_item::dialog::Dialog => Dialog, synced as Clone::clone;
    mcrs_minecraft_item::damage_type::DamageType => DamageType, synced as Clone::clone;
    mcrs_minecraft_item::block_transformer::BlockTransformer => BlockTransformer, synced as Clone::clone;
    mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern => DecoratedPotPattern, synced as Clone::clone;
    WolfSoundVariant => WolfSoundVariant [non_empty],
        synced as Clone::clone;
    PigSoundVariant => PigSoundVariant [non_empty],
        synced as Clone::clone;
    CowSoundVariant => CowSoundVariant [non_empty],
        synced as Clone::clone;
    ChickenSoundVariant => ChickenSoundVariant [non_empty],
        synced as Clone::clone;
    CatSoundVariant => CatSoundVariant [non_empty],
        synced as Clone::clone;
    WorldClock => WorldClock, synced as Clone::clone;
    Timeline => Timeline, synced as |timeline| NetworkTimeline::from(timeline);
    crate::sulfur_cube_archetype::SulfurCubeArchetype => SulfurCubeArchetype, synced as Clone::clone;
    mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider => DirectBlockStateProvider, synced as Clone::clone;
    mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList => MultiNoiseBiomeSourceParameterList;
    crate::worldgen::world_preset::WorldPreset => WorldPreset;
    crate::enchantment_provider::EnchantmentProvider => EnchantmentProvider;
    crate::villager_trade::VillagerTrade => VillagerTrade;
    crate::villager_trade::TradeSet => TradeSet;
}

pub fn world_registries(datapack_report: &[u8]) -> Result<WorldRegistries, LoadReport> {
    let mut world =
        WorldRegistries::from_datapack_report(datapack_report).map_err(LoadReport::invalid)?;
    let mut undeclared = LoadReport::new();
    parse_world_registries(&mut world, &mut undeclared);
    parse_split_registries(&mut world, &mut undeclared);
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
    if world.parses(
        mcrs_minecraft_biome::keys::MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST
            .location()
            .as_static_str(),
    ) {
        world.validate::<MultiNoiseBiomeSourceParameterList>(
            mcrs_minecraft_biome::keys::MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST.location(),
            check_parameter_list_biomes,
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
        built: if vanilla {
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
    world.load(&statics, &packs)
}

pub fn register_loaded<T: 'static, N: Serialize>(
    access: &mut RegistryAccess,
    set: &RegistrySet,
    registry: &str,
    project: fn(&T) -> N,
) {
    let values = set
        .column::<T>(registry)
        .unwrap_or_else(|| panic!("{registry} holds no values of the projected type"));
    register_projected(access, set, registry, |id| project(&values[id]));
}

fn register_joined<T, P: Parts, N: Serialize>(
    access: &mut RegistryAccess,
    set: &RegistrySet,
    registry: &str,
    join: for<'a> fn(P::Refs<'a>) -> T,
    project: fn(&T) -> N,
) {
    register_projected(access, set, registry, |id| {
        let parts = P::refs(set, registry, id)
            .unwrap_or_else(|| panic!("{registry} holds no split columns of the joined type"));
        project(&join(parts))
    });
}

fn register_projected<N: Serialize>(
    access: &mut RegistryAccess,
    set: &RegistrySet,
    registry: &str,
    project: impl Fn(usize) -> N,
) {
    let table = set
        .table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"));
    let entries = set.scope(|| {
        table
            .names()
            .iter()
            .enumerate()
            .map(|(id, name)| {
                let tag = mcrs_minecraft_nbt::to_nbt_tag(&project(id)).unwrap_or_else(|e| {
                    panic!("{registry}/{name} does not encode for the network: {e}")
                });
                RegistryEntry {
                    location: name.clone(),
                    data: Some(tag),
                    pack_source: (set.pack_of(registry, id) == Some(VANILLA_PACK))
                        .then(PackSource::vanilla_core),
                }
            })
            .collect()
    });
    access.register(SyncedRegistry::from_registry_entries(registry, entries));
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
            .registry::<keys::EntityType>()
            .expect("the registries report holds minecraft:entity_type"),
    );
    world.insert_resource(registries.clone());
}

pub fn share_registries(world: &mut World) {
    share::<RegistrySet>(world);
    share::<RegistryAccess>(world);
    share::<Blocks>(world);
    share::<Items>(world);
    share::<Registry<EnchantmentData>>(world);
    share::<Entries<EnchantmentData, EnchantmentData>>(world);
    share::<Entries<EnchantmentData, Option<EnchantmentEffects>>>(world);
    share::<Registry<Biome>>(world);
    share::<Registry<Structure>>(world);
    share::<Registry<Timeline>>(world);
    share::<Registry<keys::EntityType>>(world);
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
    use super::*;
    use mcrs_minecraft_core::registry_key::RegistryKey;

    #[derive(serde::Deserialize, Serialize)]
    struct Probe {
        asset_id: String,
    }

    impl Probe {
        const KEY: RegistryKey<Probe> =
            RegistryKey::new(mcrs_minecraft_core::rl!("minecraft:test_variant"));
    }

    #[test]
    fn only_an_entry_from_the_vanilla_pack_claims_the_vanilla_source() {
        let file = |path: &str| PackFile {
            path: path.to_owned(),
            bytes: Some(br#"{"asset_id":"x"}"#.to_vec()),
        };
        let packs = [
            Pack {
                name: VANILLA_PACK.to_owned(),
                files: vec![file("minecraft/test_variant/a.json")],
                built: Vec::new(),
            },
            Pack {
                name: "extra".to_owned(),
                files: vec![file("minecraft/test_variant/b.json")],
                built: Vec::new(),
            },
        ];
        let mut registries = WorldRegistries::new([mcrs_minecraft_core::ResourceLocation::from(
            Probe::KEY.location(),
        )]);
        registries.parse::<Probe>(Probe::KEY.location());
        let set = registries.load(&RegistrySet::new(), &packs).unwrap();

        let mut access = RegistryAccess::default();
        register_loaded::<Probe, _>(&mut access, &set, Probe::KEY.location().as_str(), |probe| {
            probe.asset_id.clone()
        });
        let claimed: Vec<_> = access
            .iter()
            .next()
            .unwrap()
            .iter_entries()
            .map(|entry| (entry.location.as_str(), entry.pack_source.is_some()))
            .collect();
        assert_eq!(claimed, [("minecraft:a", true), ("minecraft:b", false)]);
    }

    #[test]
    fn generated_names_follow_the_report_order() {
        let set = mcrs_minecraft_registry::static_report::from_report(include_bytes!(
            "../../../assets/mcrs/reports/registries.json"
        ))
        .unwrap();
        let registries: [(&str, &[mcrs_minecraft_core::StaticResourceLocation]); 6] = [
            ("minecraft:attribute", keys::attribute::ENTRIES),
            ("minecraft:block", keys::block::ENTRIES),
            (
                "minecraft:block_entity_type",
                keys::block_entity_type::ENTRIES,
            ),
            ("minecraft:entity_type", keys::entity_type::ENTRIES),
            ("minecraft:item", keys::item::ENTRIES),
            ("minecraft:menu", keys::menu::ENTRIES),
        ];
        for (registry, names) in registries {
            let table = set.table(registry).unwrap();
            let in_report: Vec<String> = table.names().iter().map(ToString::to_string).collect();
            let generated: Vec<String> = names.iter().map(ToString::to_string).collect();
            assert_eq!(in_report, generated, "{registry}");
        }
    }

    #[test]
    fn generated_constants_stand_at_the_entry_they_name() {
        use keys::entity_type as kind;
        let entity_types = [
            (kind::ALLAY, "allay"),
            (kind::CAT, "cat"),
            (kind::CHEST_MINECART, "chest_minecart"),
            (kind::CHICKEN, "chicken"),
            (kind::DROWNED, "drowned"),
            (kind::ELDER_GUARDIAN, "elder_guardian"),
            (kind::EVOKER, "evoker"),
            (kind::ITEM, "item"),
            (kind::ITEM_FRAME, "item_frame"),
            (kind::PLAYER, "player"),
            (kind::SHULKER, "shulker"),
            (kind::TNT, "tnt"),
            (kind::VILLAGER, "villager"),
            (kind::VINDICATOR, "vindicator"),
            (kind::WITCH, "witch"),
            (kind::ZOMBIE_NAUTILUS, "zombie_nautilus"),
            (kind::ZOMBIE_VILLAGER, "zombie_villager"),
        ];
        for (key, name) in entity_types {
            assert_eq!(key.as_static_str(), format!("minecraft:{name}"));
            assert_eq!(keys::entity_type::ENTRIES[key.id().index()], key.location());
        }
        assert_eq!(keys::block::TNT.as_static_str(), "minecraft:tnt");
        assert_eq!(
            keys::block::ENTRIES[keys::block::TNT.id().index()],
            keys::block::TNT.location()
        );
        assert_eq!(
            keys::attribute::MAX_HEALTH.as_static_str(),
            "minecraft:max_health"
        );
    }
}
