use crate::block_transformer::BlockTransformer;
use crate::chat_type::ChatType;
use crate::damage_type::DamageType;
use crate::data_pack::walk_files;
use crate::decorated_pot_pattern::DecoratedPotPattern;
use crate::dialog::Dialog;
use crate::enchantment_provider::EnchantmentProvider;
use crate::entity::minecraft::EntityIds;
use crate::sulfur_cube_archetype::SulfurCubeArchetype;
use crate::test_types::{TestEnvironment, TestInstance};
use crate::variant;
use crate::villager_trade::{TradeSet, VillagerTrade};
use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::{AssetSourceId, ErasedAssetReader};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use bevy_ecs::world::World;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::{PACKS_ROOT, VANILLA_PACK, layered_file_source, pack_names};
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_assets::{PackSource, RegistryAccess, RegistryEntry, RegistrySnapshotErased};
use mcrs_minecraft_biome::parameter_list::{
    MultiNoiseBiomeSourceParameterList, check_parameter_list_biomes,
};
use mcrs_minecraft_biome::{Biome, NetworkBiome};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_dimension::dimension_type::{DimensionType, NetworkDimensionType};
use mcrs_minecraft_environment::timeline::{NetworkTimeline, Timeline};
use mcrs_minecraft_environment::world_clock::{WorldClock, check_time_markers};
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, Items, JukeboxSong, PaintingVariantValue, TrimMaterial,
    TrimPattern,
};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_keys::{Block, Enchantment, Item};
use mcrs_minecraft_registry::shared::share;
use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{
    Entries, LoadReport, Pack, PackFile, Registry, RegistrySet, WorldRegistries,
};
use mcrs_minecraft_worldgen_feature::tree::DirectBlockStateProvider;
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
                    if world.parses(<$key as RegistryKey>::KEY.as_str()) {
                        world.$non_empty(<$key as RegistryKey>::KEY);
                    }
                )?
            )*
        }

        pub fn register_world_registries(access: &mut RegistryAccess, set: &RegistrySet) {
            $($(
                register_loaded::<$value, _>(
                    access,
                    set,
                    <$key as RegistryKey>::KEY.as_str(),
                    $project,
                );
            )?)*
        }
    };
}

world_registry_table! {
    keys::BannerPattern => BannerPattern, synced as Clone::clone;
    keys::Instrument => InstrumentValue, synced as Clone::clone;
    keys::JukeboxSong => JukeboxSong, synced as Clone::clone;
    keys::PaintingVariant => PaintingVariantValue [non_empty], synced as Clone::clone;
    keys::TrimMaterial => TrimMaterial, synced as Clone::clone;
    keys::TrimPattern => TrimPattern, synced as Clone::clone;
    keys::ChatType => ChatType, synced as Clone::clone;
    keys::TestEnvironment => TestEnvironment, synced as Clone::clone;
    keys::TestInstance => TestInstance, synced as Clone::clone;
    keys::Dialog => Dialog, synced as Clone::clone;
    keys::DamageType => DamageType, synced as Clone::clone;
    keys::BlockTransformer => BlockTransformer, synced as Clone::clone;
    keys::Enchantment => EnchantmentData, synced as Clone::clone;
    keys::DecoratedPotPattern => DecoratedPotPattern, synced as Clone::clone;
    keys::WolfVariant => variant::WolfVariant [non_empty],
        synced as |v| variant::NetworkWolfVariant::from(v);
    keys::WolfSoundVariant => variant::WolfSoundVariant [non_empty],
        synced as Clone::clone;
    keys::PigVariant => variant::PigVariant [non_empty],
        synced as |v| variant::NetworkPigVariant::from(v);
    keys::PigSoundVariant => variant::PigSoundVariant [non_empty],
        synced as Clone::clone;
    keys::CowVariant => variant::CowVariant [non_empty],
        synced as |v| variant::NetworkCowVariant::from(v);
    keys::CowSoundVariant => variant::CowSoundVariant [non_empty],
        synced as Clone::clone;
    keys::ChickenVariant => variant::ChickenVariant [non_empty],
        synced as |v| variant::NetworkChickenVariant::from(v);
    keys::ChickenSoundVariant => variant::ChickenSoundVariant [non_empty],
        synced as Clone::clone;
    keys::CatVariant => variant::CatVariant [non_empty],
        synced as |v| variant::NetworkCatVariant::from(v);
    keys::CatSoundVariant => variant::CatSoundVariant [non_empty],
        synced as Clone::clone;
    keys::FrogVariant => variant::FrogVariant [non_empty],
        synced as |v| variant::NetworkFrogVariant::from(v);
    keys::ZombieNautilusVariant => variant::ZombieNautilusVariant [non_empty],
        synced as |v| variant::NetworkZombieNautilusVariant::from(v);
    keys::WorldClock => WorldClock, synced as Clone::clone;
    keys::Timeline => Timeline, synced as |timeline| NetworkTimeline::from(timeline);
    keys::SulfurCubeArchetype => SulfurCubeArchetype, synced as Clone::clone;
    keys::Biome => Biome, synced as |biome| NetworkBiome::from(biome);
    keys::DimensionType => DimensionType, synced as |d| NetworkDimensionType::from(d);
    keys::BlockStateProvider => DirectBlockStateProvider, synced as Clone::clone;
    keys::MultiNoiseBiomeSourceParameterList => MultiNoiseBiomeSourceParameterList;
    keys::EnchantmentProvider => EnchantmentProvider;
    keys::VillagerTrade => VillagerTrade;
    keys::TradeSet => TradeSet;
}

pub fn world_registries(datapack_report: &[u8]) -> Result<WorldRegistries, LoadReport> {
    let mut world =
        WorldRegistries::from_datapack_report(datapack_report).map_err(LoadReport::invalid)?;
    let mut undeclared = LoadReport::new();
    parse_world_registries(&mut world, &mut undeclared);
    if world.parses(keys::Timeline::KEY.as_str()) {
        world.validate::<Timeline>(keys::Timeline::KEY, check_time_markers);
    }
    if world.parses(keys::MultiNoiseBiomeSourceParameterList::KEY.as_str()) {
        world.validate::<MultiNoiseBiomeSourceParameterList>(
            keys::MultiNoiseBiomeSourceParameterList::KEY,
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
    K: RegistryKey,
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    if world
        .declared()
        .any(|declared| declared.as_str() == K::KEY.as_str())
    {
        world.parse::<T>(K::KEY);
    } else {
        report.invalid_report(format_args!(
            "{} is not a world registry of the data pack report",
            K::KEY
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
                    found.entry(path).or_default();
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
    let table = set
        .table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"));
    let values = set
        .column::<T>(registry)
        .unwrap_or_else(|| panic!("{registry} holds no values of the projected type"));
    let entries = set.scope(|| {
        table
            .names()
            .iter()
            .zip(values)
            .enumerate()
            .map(|(id, (name, value))| {
                let tag = mcrs_minecraft_nbt::to_nbt_tag(&project(value)).unwrap_or_else(|e| {
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
    access.register(RegistrySnapshotErased::from_registry_entries(
        registry, entries,
    ));
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
        let source = asset_server
            .get_source(AssetSourceId::Default)
            .expect("default AssetSource missing");
        let bytes = bevy_tasks::block_on(read_whole(
            source.reader(),
            Path::new("mcrs/reports/registries.json"),
        ))
        .expect("the registries report reads");
        let (statics, _) = static_registries(&bytes).unwrap_or_else(|report| refuse(&report));
        load_registries(&asset_server, statics).unwrap_or_else(|report| refuse(&report))
    });
    &SET
}

pub fn share_registries(world: &mut World) {
    share::<RegistrySet>(world);
    share::<EntityIds>(world);
    share::<RegistryAccess>(world);
    share::<Blocks>(world);
    share::<Items>(world);
    share::<Registry<Enchantment>>(world);
    share::<Entries<Enchantment, EnchantmentData>>(world);
    share::<mcrs_minecraft_worldgen::tables::WorldgenTables>(world);
    share::<DynTagRegistry<Block>>(world);
    share::<DynTagRegistry<Item>>(world);
}

pub fn static_registries(report: &[u8]) -> Result<(RegistrySet, EntityIds), LoadReport> {
    let set = from_report(report).map_err(LoadReport::invalid)?;
    let mut missing = LoadReport::new();
    missing.registry::<keys::SoundEvent>(&set);
    missing.registry::<Block>(&set);
    missing.registry::<Item>(&set);
    let entity_ids = EntityIds::resolve(&set, &mut missing);
    match entity_ids {
        Some(entity_ids) if missing.is_empty() => Ok((set, entity_ids)),
        _ => Err(missing),
    }
}

pub fn refuse(report: &LoadReport) -> ! {
    tracing::error!("the registries are unusable:\n{report}");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_malformed_report_is_refused_with_a_report() {
        let json = br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":0},"a:b":{"protocol_id":0}}}}"#;
        let report = static_registries(json)
            .err()
            .expect("a shared id is refused");
        assert!(!report.is_empty());
        let text = report.to_string();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("minecraft:x"), "{text}");
    }

    fn refused_without(registry: &str) -> LoadReport {
        let mut report: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../assets/mcrs/reports/registries.json"
        ))
        .unwrap();
        report
            .as_object_mut()
            .unwrap()
            .remove(registry)
            .unwrap_or_else(|| panic!("the report carries {registry}"));
        let bytes = serde_json::to_vec(&report).unwrap();

        static_registries(&bytes)
            .err()
            .unwrap_or_else(|| panic!("a report without {registry} is refused"))
    }

    #[test]
    fn a_report_without_a_required_static_registry_is_refused() {
        for registry in ["minecraft:sound_event", "minecraft:block", "minecraft:item"] {
            let refused = refused_without(registry);
            assert!(refused.to_string().contains(registry), "{refused}");
        }
    }
}
