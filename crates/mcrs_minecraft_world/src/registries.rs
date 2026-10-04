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
use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::{AssetSourceId, ErasedAssetReader};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::{PACKS_ROOT, VANILLA_PACK, layered_file_source, pack_names};
use mcrs_minecraft_assets::{PackSource, RegistryAccess, RegistryEntry, RegistrySnapshotErased};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, Item, JukeboxSong, PaintingVariantValue, SoundEvent,
    TrimMaterial, TrimPattern,
};
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{LoadReport, Pack, PackFile, RegistrySet, WorldRegistries};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::LazyLock;

pub fn world_registries(datapack_report: &[u8]) -> Result<WorldRegistries, LoadReport> {
    let mut world = WorldRegistries::from_datapack_report(datapack_report).map_err(|error| {
        let mut report = LoadReport::new();
        report.invalid_report(error);
        report
    })?;
    let mut undeclared = LoadReport::new();
    parse::<BannerPattern>(&mut world, &mut undeclared);
    parse::<InstrumentValue>(&mut world, &mut undeclared);
    parse::<JukeboxSong>(&mut world, &mut undeclared);
    parse_required::<PaintingVariantValue, PaintingVariantValue>(&mut world, &mut undeclared);
    parse::<TrimMaterial>(&mut world, &mut undeclared);
    parse::<TrimPattern>(&mut world, &mut undeclared);
    parse::<ChatType>(&mut world, &mut undeclared);
    parse::<TestEnvironment>(&mut world, &mut undeclared);
    parse::<TestInstance>(&mut world, &mut undeclared);
    parse_as::<mcrs_minecraft_registry::key::Dialog, Dialog>(&mut world, &mut undeclared);
    parse::<EnchantmentData>(&mut world, &mut undeclared);
    parse::<EnchantmentProvider>(&mut world, &mut undeclared);
    parse::<SulfurCubeArchetype>(&mut world, &mut undeclared);
    parse_as::<mcrs_minecraft_entity::DamageType, DamageType>(&mut world, &mut undeclared);
    parse_as::<mcrs_minecraft_registry::key::DecoratedPotPattern, DecoratedPotPattern>(
        &mut world,
        &mut undeclared,
    );
    parse_as::<mcrs_minecraft_registry::key::BlockTransformer, BlockTransformer>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::WolfVariant, variant::WolfVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::WolfSoundVariant, variant::WolfSoundVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::PigVariant, variant::PigVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::PigSoundVariant, variant::PigSoundVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::CowVariant, variant::CowVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::CowSoundVariant, variant::CowSoundVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::ChickenVariant, variant::ChickenVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::ChickenSoundVariant, variant::ChickenSoundVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::CatVariant, variant::CatVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::CatSoundVariant, variant::CatSoundVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::FrogVariant, variant::FrogVariant>(
        &mut world,
        &mut undeclared,
    );
    parse_required::<mcrs_minecraft_entity::ZombieNautilusVariant, variant::ZombieNautilusVariant>(
        &mut world,
        &mut undeclared,
    );
    if undeclared.is_empty() {
        Ok(world)
    } else {
        Err(undeclared)
    }
}

fn parse<T>(world: &mut WorldRegistries, report: &mut LoadReport)
where
    T: RegistryKey + DeserializeOwned + Serialize + Send + Sync + 'static,
{
    parse_as::<T, T>(world, report);
}

/// `K` names the registry whose entries are read as `T`, for a value type that
/// lives where its key type cannot.
fn parse_as<K, T>(world: &mut WorldRegistries, report: &mut LoadReport)
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

fn parse_required<K, T>(world: &mut WorldRegistries, report: &mut LoadReport)
where
    K: RegistryKey,
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    parse_as::<K, T>(world, report);
    if world.parses(K::KEY.as_str()) {
        world.non_empty(K::KEY);
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
                    found.entry(path).or_default();
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
    let bytes = bevy_tasks::block_on(read_whole(source.reader(), path)).map_err(|error| {
        let mut report = LoadReport::new();
        report.invalid_report(format_args!("{}: {error}", path.display()));
        report
    })?;
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
            layered_file_source(&AssetPlugin::default().file_path),
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

pub fn static_registries(report: &[u8]) -> Result<(RegistrySet, EntityIds), LoadReport> {
    let set = from_report(report).map_err(|error| {
        let mut report = LoadReport::new();
        report.invalid_report(error);
        report
    })?;
    let mut missing = LoadReport::new();
    missing.registry::<SoundEvent>(&set);
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
    fn a_broken_corpus_is_refused_with_registry_entry_and_file() {
        use mcrs_minecraft_registry::{Pack, PackFile};

        let report = br#"{"others":{},"registries":{
            "minecraft:banner_pattern":{"elements":true,"stable":false,"tags":true},
            "minecraft:instrument":{"elements":true,"stable":false,"tags":true},
            "minecraft:jukebox_song":{"elements":true,"stable":false,"tags":true},
            "minecraft:painting_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:trim_material":{"elements":true,"stable":false,"tags":true},
            "minecraft:trim_pattern":{"elements":true,"stable":false,"tags":true},
            "minecraft:damage_type":{"elements":true,"stable":false,"tags":true},
            "minecraft:decorated_pot_pattern":{"elements":true,"stable":false,"tags":true},
            "minecraft:block_transformer":{"elements":true,"stable":false,"tags":true},
            "minecraft:wolf_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:wolf_sound_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:pig_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:pig_sound_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:cow_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:cow_sound_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:chicken_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:chicken_sound_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:cat_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:cat_sound_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:frog_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:zombie_nautilus_variant":{"elements":true,"stable":false,"tags":true},
            "minecraft:chat_type":{"elements":true,"stable":false,"tags":true},
            "minecraft:test_environment":{"elements":true,"stable":false,"tags":true},
            "minecraft:test_instance":{"elements":true,"stable":false,"tags":true},
            "minecraft:dialog":{"elements":true,"stable":false,"tags":true},
            "minecraft:enchantment":{"elements":true,"stable":false,"tags":true},
            "minecraft:enchantment_provider":{"elements":true,"stable":false,"tags":true},
            "minecraft:sulfur_cube_archetype":{"elements":true,"stable":false,"tags":true}}}"#;
        let world = world_registries(report).expect("the report parses");
        let packs = [Pack {
            name: VANILLA_PACK.to_owned(),
            files: vec![PackFile {
                path: "minecraft/banner_pattern/base.json".to_owned(),
                bytes: Some(
                    br#"{"asset_id":"minecraft:base","translation_key":"k","extra":1}"#.to_vec(),
                ),
            }],
        }];
        let refused = world
            .load(&RegistrySet::new(), &packs)
            .err()
            .expect("a file with an unknown field is refused");
        let text = refused.to_string();
        for part in [
            "minecraft:banner_pattern",
            "minecraft:base",
            "minecraft/banner_pattern/base.json",
            "extra",
        ] {
            assert!(text.contains(part), "{part} missing from:\n{text}");
        }
    }

    #[test]
    fn a_report_without_sound_events_is_refused() {
        let refused = refused_without("minecraft:sound_event");
        assert!(
            refused.to_string().contains("minecraft:sound_event"),
            "{refused}"
        );
    }

    #[test]
    fn a_report_without_blocks_is_refused() {
        let refused = refused_without("minecraft:block");
        assert!(refused.to_string().contains("minecraft:block"), "{refused}");
    }
}
