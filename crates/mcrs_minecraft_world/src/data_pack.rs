use crate::{
    LoadedRegistryAssets, banner_pattern, chat_type, damage_type, dialog, entity, instrument,
    jukebox_song, painting_variant, sound, test_types, variant,
};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{Asset, AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::snapshot::rl_from_asset_path;
use mcrs_minecraft_assets::tag::file::TagFile;
use mcrs_minecraft_assets::tag::{DynTagLoader, TagLoader, TagLoadersSettled};
use mcrs_minecraft_biome as biome;
use mcrs_minecraft_block as block;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use mcrs_minecraft_dimension::dimension_type::DimensionType;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::{timeline, world_clock};
use mcrs_minecraft_item::enchantment::data::EnchantmentData;
use mcrs_minecraft_registry::DynRegistryIndex;
use mcrs_minecraft_registry::StaticRegistry;
use {mcrs_minecraft_item as item, mcrs_minecraft_item::trim};

pub(crate) fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

// File listings baked from `assets/` at build time. Used as the fallback
// manifest when the active `AssetSource` cannot enumerate directories
// (HTTP/WASM, embedded packs without an index, etc.).
mod registry_files {
    include!(concat!(env!("OUT_DIR"), "/registry_files.rs"));
}

/// Resolve the set of `<folder>/<file>.<extension>` paths that should be loaded
/// for a registry folder.
///
/// First tries `AssetReader::read_directory` on the default `AssetSource` —
/// this picks up files that the active source can enumerate, including
/// future resource packs mounted as file-system folders or ZIPs.
/// Falls back to the build-time manifest baked from the vanilla `assets/`
/// tree for sources that cannot list directories (HTTP/WASM).
fn list_registry_files(
    asset_server: &AssetServer,
    folder: &str,
    extension: &str,
    fallback: &'static [&'static str],
) -> Vec<String> {
    let dynamic: Vec<String> = match asset_server.get_source(AssetSourceId::Default) {
        Ok(source) => {
            let reader = source.reader();
            bevy_tasks::block_on(walk_files(reader, std::path::PathBuf::from(folder)))
                .into_iter()
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some(extension))
                .filter_map(|p| p.to_str().map(str::to_owned))
                .collect()
        }
        Err(err) => {
            tracing::warn!(folder, %err, "default AssetSource missing");
            Vec::new()
        }
    };

    if !dynamic.is_empty() {
        let mut sorted = dynamic;
        sorted.sort();
        return sorted;
    }

    fallback.iter().map(|s| (*s).to_owned()).collect()
}

fn request_registry<T: Asset>(
    asset_server: &AssetServer,
    loaded: &mut LoadedRegistryAssets,
    folder: &str,
    extension: &str,
    fallback: &'static [&'static str],
) {
    let files = list_registry_files(asset_server, folder, extension, fallback);
    let count = files.len();
    for path in files {
        loaded.handles.push(asset_server.load::<T>(path).untyped());
    }
    tracing::info!(
        folder,
        count,
        kind = std::any::type_name::<T>(),
        "requested registry assets"
    );
}

pub(crate) fn request_data_pack_assets(
    asset_server: Res<AssetServer>,
    mut loaded: ResMut<LoadedRegistryAssets>,
) {
    use registry_files::*;
    request_registry::<biome::Biome>(
        &asset_server,
        &mut loaded,
        FOLDER_BIOME,
        "json",
        FILES_BIOME,
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::CarverConfigAsset>(
        &asset_server,
        &mut loaded,
        FOLDER_CARVER,
        "json",
        FILES_CARVER,
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::FeatureAsset>(
        &asset_server,
        &mut loaded,
        FOLDER_FEATURE,
        "json",
        FILES_FEATURE,
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::PlacedFeatureAsset>(
        &asset_server,
        &mut loaded,
        FOLDER_PLACED_FEATURE,
        "json",
        FILES_PLACED_FEATURE,
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::StructureSetAsset>(
        &asset_server,
        &mut loaded,
        FOLDER_STRUCTURE_SET,
        "json",
        FILES_STRUCTURE_SET,
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::StructureAsset>(
        &asset_server,
        &mut loaded,
        FOLDER_STRUCTURE,
        "json",
        FILES_STRUCTURE,
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::TemplatePoolAsset>(
        &asset_server,
        &mut loaded,
        FOLDER_TEMPLATE_POOL,
        "json",
        FILES_TEMPLATE_POOL,
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::TemplateAsset>(
        &asset_server,
        &mut loaded,
        FOLDER_TEMPLATE,
        "nbt",
        FILES_TEMPLATE,
    );
    request_registry::<mcrs_minecraft_dimension::dimension_type::DimensionType>(
        &asset_server,
        &mut loaded,
        FOLDER_DIMENSION_TYPE,
        "json",
        FILES_DIMENSION_TYPE,
    );
    request_registry::<chat_type::ChatType>(
        &asset_server,
        &mut loaded,
        FOLDER_CHAT_TYPE,
        "json",
        FILES_CHAT_TYPE,
    );
    request_registry::<trim::TrimPattern>(
        &asset_server,
        &mut loaded,
        FOLDER_TRIM_PATTERN,
        "json",
        FILES_TRIM_PATTERN,
    );
    request_registry::<trim::TrimMaterial>(
        &asset_server,
        &mut loaded,
        FOLDER_TRIM_MATERIAL,
        "json",
        FILES_TRIM_MATERIAL,
    );
    request_registry::<variant::WolfVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_WOLF_VARIANT,
        "json",
        FILES_WOLF_VARIANT,
    );
    request_registry::<variant::WolfSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_WOLF_SOUND_VARIANT,
        "json",
        FILES_WOLF_SOUND_VARIANT,
    );
    request_registry::<variant::PigSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_PIG_SOUND_VARIANT,
        "json",
        FILES_PIG_SOUND_VARIANT,
    );
    request_registry::<variant::CatSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CAT_SOUND_VARIANT,
        "json",
        FILES_CAT_SOUND_VARIANT,
    );
    request_registry::<variant::CowSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_COW_SOUND_VARIANT,
        "json",
        FILES_COW_SOUND_VARIANT,
    );
    request_registry::<variant::ChickenSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CHICKEN_SOUND_VARIANT,
        "json",
        FILES_CHICKEN_SOUND_VARIANT,
    );
    request_registry::<variant::PigVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_PIG_VARIANT,
        "json",
        FILES_PIG_VARIANT,
    );
    request_registry::<variant::FrogVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_FROG_VARIANT,
        "json",
        FILES_FROG_VARIANT,
    );
    request_registry::<variant::CatVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CAT_VARIANT,
        "json",
        FILES_CAT_VARIANT,
    );
    request_registry::<variant::CowVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_COW_VARIANT,
        "json",
        FILES_COW_VARIANT,
    );
    request_registry::<variant::ChickenVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CHICKEN_VARIANT,
        "json",
        FILES_CHICKEN_VARIANT,
    );
    request_registry::<variant::ZombieNautilusVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_ZOMBIE_NAUTILUS_VARIANT,
        "json",
        FILES_ZOMBIE_NAUTILUS_VARIANT,
    );
    request_registry::<painting_variant::PaintingVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_PAINTING_VARIANT,
        "json",
        FILES_PAINTING_VARIANT,
    );
    request_registry::<damage_type::DamageType>(
        &asset_server,
        &mut loaded,
        FOLDER_DAMAGE_TYPE,
        "json",
        FILES_DAMAGE_TYPE,
    );
    request_registry::<banner_pattern::BannerPattern>(
        &asset_server,
        &mut loaded,
        FOLDER_BANNER_PATTERN,
        "json",
        FILES_BANNER_PATTERN,
    );
    request_registry::<jukebox_song::JukeboxSong>(
        &asset_server,
        &mut loaded,
        FOLDER_JUKEBOX_SONG,
        "json",
        FILES_JUKEBOX_SONG,
    );
    request_registry::<instrument::Instrument>(
        &asset_server,
        &mut loaded,
        FOLDER_INSTRUMENT,
        "json",
        FILES_INSTRUMENT,
    );
    request_registry::<dialog::Dialog>(
        &asset_server,
        &mut loaded,
        FOLDER_DIALOG,
        "json",
        FILES_DIALOG,
    );
    request_registry::<timeline::Timeline>(
        &asset_server,
        &mut loaded,
        FOLDER_TIMELINE,
        "json",
        FILES_TIMELINE,
    );
    request_registry::<world_clock::WorldClock>(
        &asset_server,
        &mut loaded,
        FOLDER_WORLD_CLOCK,
        "json",
        FILES_WORLD_CLOCK,
    );
    request_registry::<test_types::TestEnvironment>(
        &asset_server,
        &mut loaded,
        FOLDER_TEST_ENVIRONMENT,
        "json",
        FILES_TEST_ENVIRONMENT,
    );
    request_registry::<test_types::TestInstance>(
        &asset_server,
        &mut loaded,
        FOLDER_TEST_INSTANCE,
        "json",
        FILES_TEST_INSTANCE,
    );
}

/// Every tag file the mounted packs ship for one registry, as
/// `(tag location, asset path)`, discovered by walking each namespace's
/// `<namespace>/tags/<registry_path>` tree through the active `AssetSource`.
///
/// A pack that adds a tag file is picked up without a code change, and a tag
/// no Rust constant names is still reachable.
pub fn list_tag_files(
    asset_server: &AssetServer,
    registry_path: &str,
) -> Vec<(ResourceLocation<std::sync::Arc<str>>, String)> {
    let Ok(source) = asset_server.get_source(AssetSourceId::Default) else {
        tracing::warn!(registry_path, "default AssetSource missing");
        return Vec::new();
    };
    let reader = source.reader();
    let mut found = Vec::new();

    bevy_tasks::block_on(async {
        let Ok(mut namespaces) = reader.read_directory(std::path::Path::new("")).await else {
            return;
        };
        let mut roots = Vec::new();
        while let Some(namespace) = namespaces.next().await {
            roots.push(namespace.join("tags").join(registry_path));
        }
        for root in roots {
            for path in walk_files(reader, root.clone()).await {
                let Some(location) = tag_location(&root, &path) else {
                    continue;
                };
                let Some(asset_path) = path.to_str() else {
                    continue;
                };
                found.push((location, asset_path.to_owned()));
            }
        }
    });

    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}

async fn walk_files(
    reader: &dyn bevy_asset::io::ErasedAssetReader,
    root: std::path::PathBuf,
) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(directory) = stack.pop() {
        let Ok(mut entries) = reader.read_directory(&directory).await else {
            continue;
        };
        while let Some(path) = entries.next().await {
            if reader.is_directory(&path).await.unwrap_or(false) {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn request_every_block_tag(
    mut loader: ResMut<TagLoader<block::Block, u32>>,
    asset_server: Res<AssetServer>,
) {
    request_every_tag(&mut loader, &asset_server);
}

pub(crate) fn request_every_fluid_tag(
    mut loader: ResMut<TagLoader<block::Fluid, u32>>,
    asset_server: Res<AssetServer>,
) {
    request_every_tag(&mut loader, &asset_server);
}

pub(crate) fn request_every_biome_tag(
    mut loader: ResMut<TagLoader<biome::Biome, u32>>,
    asset_server: Res<AssetServer>,
) {
    request_every_tag(&mut loader, &asset_server);
}

fn request_every_tag<T: TaggedRegistry + 'static>(
    loader: &mut TagLoader<T, u32>,
    asset_server: &AssetServer,
) {
    let files = list_tag_files(asset_server, T::REGISTRY_PATH);
    let count = files.len();
    for (location, _) in files {
        loader.request(&TagKey::<T, _>::from_location(location), asset_server);
    }
    tracing::info!(
        count,
        registry = T::REGISTRY_PATH,
        "requested every shipped tag"
    );
}

/// `minecraft/tags/block/mineable/pickaxe.json` under the root
/// `minecraft/tags/block` is `minecraft:mineable/pickaxe`.
fn tag_location(
    root: &std::path::Path,
    path: &std::path::Path,
) -> Option<mcrs_minecraft_core::resource_location::ResourceLocation<std::sync::Arc<str>>> {
    let namespace = root.iter().next()?.to_str()?;
    let relative = path.strip_prefix(root).ok()?.to_str()?;
    let name = relative.strip_suffix(".json")?;
    mcrs_minecraft_core::resource_location::ResourceLocation::parse(&format!("{namespace}:{name}"))
        .ok()
}

pub(crate) fn check_tags_ready(
    tags_settled: Res<TagLoadersSettled>,
    registry_assets: Res<LoadedRegistryAssets>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if tags_settled.get() && registry_assets.all_handles_settled(&asset_server) {
        tracing::info!("all tag files and registry assets settled — entering WorldgenFreeze");
        next.set(AppState::WorldgenFreeze);
    }
}

/// Resolve infiniburn tag files from loaded `DimensionType` assets into
/// the block `TagLoader`. The tag files were loaded as sub-assets by
/// `DimensionTypeLoader`, so they're guaranteed to be available here.
pub(crate) fn resolve_infiniburn_tags(
    mut tags: ResMut<TagLoader<block::Block, u32>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<block::definition::Blocks>,
    dim_types: Res<Assets<DimensionType>>,
) {
    let mut resolved = 0usize;
    for (_id, dim_type) in dim_types.iter() {
        let key = dim_type.infiniburn.key();
        if let Some(tf) = tag_files.get(dim_type.infiniburn.handle()) {
            tags.resolve_and_insert(key.location().clone(), tf, &tag_files, &*registry);
            resolved += 1;
        } else {
            tracing::warn!(
                "infiniburn tag file not available at WorldgenFreeze: {}",
                key.as_str()
            );
        }
    }
    if resolved > 0 {
        tracing::info!(resolved_tags = resolved, "resolved infiniburn tags");
    }
}

/// The dense id space the timeline tag bitsets are resolved against.
pub(crate) fn index_timelines(
    timelines: Res<Assets<Timeline>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let entries: Vec<_> = timelines
        .iter()
        .filter_map(|(id, _)| rl_from_asset_path(asset_server.get_path(id)?.path(), "timeline"))
        .collect();
    tracing::info!(count = entries.len(), "indexed timelines");
    commands.insert_resource(DynRegistryIndex::<Timeline>::build(entries.into_iter()));
}

pub(crate) fn index_biomes(
    biomes: Res<Assets<biome::Biome>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let entries: Vec<_> = biomes
        .iter()
        .filter_map(|(id, _)| {
            rl_from_asset_path(asset_server.get_path(id)?.path(), "worldgen/biome")
        })
        .collect();
    tracing::info!(count = entries.len(), "indexed biomes");
    commands.insert_resource(DynRegistryIndex::<biome::Biome>::build(entries.into_iter()));
}

/// Resolve the timeline tag every dimension type names. The tag files were
/// loaded as sub-assets by `DimensionTypeLoader`, so they are available here.
pub(crate) fn resolve_timeline_tags(
    mut tags: ResMut<DynTagLoader<Timeline>>,
    tag_files: Res<Assets<TagFile>>,
    index: Res<DynRegistryIndex<Timeline>>,
    dim_types: Res<Assets<DimensionType>>,
) {
    for (_id, dim_type) in dim_types.iter() {
        let Some(tag) = &dim_type.timelines else {
            continue;
        };
        match tag_files.get(tag.handle()) {
            Some(tag_file) => {
                tags.resolve_and_insert(tag.key().location().clone(), tag_file, &tag_files, &*index)
            }
            None => tracing::warn!(
                "timeline tag file not available at WorldgenFreeze: {}",
                tag.key().as_str()
            ),
        }
    }
}

pub(crate) fn register_static_registries_with_access(
    item_registry: Res<StaticRegistry<item::Item>>,
    sound_registry: Res<StaticRegistry<sound::SoundEvent>>,
    entity_registry: Res<StaticRegistry<entity::EntityType>>,
    enchantment_registry: Res<StaticRegistry<EnchantmentData>>,
    mut access: ResMut<mcrs_minecraft_assets::RegistryAccess>,
) {
    access.register(Box::new(
        mcrs_minecraft_assets::RegistrySnapshotErased::from_static(
            "minecraft:item",
            &item_registry,
            |_, _| None,
            Some(mcrs_minecraft_assets::PackSource::vanilla_core()),
        ),
    ));
    access.register(Box::new(
        mcrs_minecraft_assets::RegistrySnapshotErased::from_static(
            "minecraft:sound_event",
            &sound_registry,
            |_, _| None,
            Some(mcrs_minecraft_assets::PackSource::vanilla_core()),
        ),
    ));
    access.register(Box::new(
        mcrs_minecraft_assets::RegistrySnapshotErased::from_static(
            "minecraft:entity_type",
            &entity_registry,
            |_, _| None,
            Some(mcrs_minecraft_assets::PackSource::vanilla_core()),
        ),
    ));
    access.register(Box::new(
        mcrs_minecraft_assets::RegistrySnapshotErased::from_static(
            "minecraft:enchantment",
            &enchantment_registry,
            |_, data| {
                use mcrs_minecraft_item::enchantment::data::NetworkEnchantmentData;
                let network = NetworkEnchantmentData::from(data);
                mcrs_minecraft_nbt::to_nbt_compound(&network).ok()
            },
            Some(mcrs_minecraft_assets::PackSource::vanilla_core()),
        ),
    ));
    tracing::info!(count = access.len(), "populated RegistryAccess");
}
