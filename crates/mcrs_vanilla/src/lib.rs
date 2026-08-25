#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod attribute;
pub mod banner_pattern;
pub mod biome;
pub mod block;
pub mod chat_type;
pub mod damage_type;
pub mod dialog;
pub mod dimension;
pub mod enchantment;
pub mod entity;
pub mod environment;
pub mod explosion;
pub mod instrument;
pub mod item;
pub mod jukebox_song;
pub mod material;
pub mod painting_variant;
pub mod player_action;
pub mod save;
pub mod sound;
pub mod test_types;
pub mod timeline;
pub mod trim;
pub mod value;
pub mod variant;
pub mod world_clock;
pub mod worldgen;

use crate::block::tags as block_tags;
use crate::dimension::dimension_type::DimensionType;
use crate::enchantment::data::EnchantmentData;
use crate::enchantment::tags as enchantment_tags;
use crate::entity::tags as entity_type_tags;
use crate::environment::{DimensionEnvironments, freeze_timelines};
use crate::item::tags as item_tags;
use crate::timeline::Timeline;
use crate::world_clock::seed_world_clocks;
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{Asset, AssetApp, AssetServer, Assets, UntypedHandle};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_core::registry::snapshot::rl_from_asset_path;
use mcrs_core::tag::file::TagFile;
use mcrs_core::tag::key::TaggedRegistry;
use mcrs_core::tag::{
    DynRegistryIndex, DynTagLoader, TagLoader, TagLoadersSettled, TagPhase, TagRegistryAppExt,
};
use mcrs_core::{AppState, ResourceLocation, StaticRegistry};

#[derive(Resource, Default)]
pub struct LoadedRegistryAssets {
    handles: Vec<UntypedHandle>,
}

impl LoadedRegistryAssets {
    /// True once every handle has either finished loading successfully or
    /// failed to load. Missing or malformed files do not stall the gate;
    /// they are logged once `WorldgenFreeze` proceeds.
    pub fn all_handles_settled(&self, asset_server: &AssetServer) -> bool {
        use bevy_asset::LoadState;
        self.handles.iter().all(|h| {
            matches!(
                asset_server.load_state(h.id()),
                LoadState::Loaded | LoadState::Failed(_)
            )
        })
    }
}

pub struct MinecraftCorePlugin;

impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<dimension::dimension_type::DimensionType>();
        app.register_asset_loader(dimension::dimension_type::DimensionTypeLoader);
        app.init_asset::<biome::Biome>();
        app.register_asset_loader(biome::BiomeLoader);
        app.init_asset::<worldgen::structure_set::StructureSet>();
        app.init_asset::<variant::WolfVariant>();
        app.register_asset_loader(variant::WolfVariantLoader);
        app.init_asset::<variant::WolfSoundVariant>();
        app.register_asset_loader(variant::WolfSoundVariantLoader);
        app.init_asset::<variant::PigSoundVariant>();
        app.register_asset_loader(variant::PigSoundVariantLoader);
        app.init_asset::<variant::CatSoundVariant>();
        app.register_asset_loader(variant::CatSoundVariantLoader);
        app.init_asset::<variant::CowSoundVariant>();
        app.register_asset_loader(variant::CowSoundVariantLoader);
        app.init_asset::<variant::ChickenSoundVariant>();
        app.register_asset_loader(variant::ChickenSoundVariantLoader);
        app.init_asset::<variant::PigVariant>();
        app.register_asset_loader(variant::PigVariantLoader);
        app.init_asset::<variant::FrogVariant>();
        app.register_asset_loader(variant::FrogVariantLoader);
        app.init_asset::<variant::CatVariant>();
        app.register_asset_loader(variant::CatVariantLoader);
        app.init_asset::<variant::CowVariant>();
        app.register_asset_loader(variant::CowVariantLoader);
        app.init_asset::<variant::ChickenVariant>();
        app.register_asset_loader(variant::ChickenVariantLoader);
        app.init_asset::<variant::ZombieNautilusVariant>();
        app.register_asset_loader(variant::ZombieNautilusVariantLoader);
        app.init_asset::<trim::TrimPattern>();
        app.register_asset_loader(trim::TrimPatternLoader);
        app.init_asset::<trim::TrimMaterial>();
        app.register_asset_loader(trim::TrimMaterialLoader);
        app.init_asset::<damage_type::DamageType>();
        app.register_asset_loader(damage_type::DamageTypeLoader);
        app.init_asset::<painting_variant::PaintingVariant>();
        app.register_asset_loader(painting_variant::PaintingVariantLoader);
        app.init_asset::<banner_pattern::BannerPattern>();
        app.register_asset_loader(banner_pattern::BannerPatternLoader);
        app.init_asset::<jukebox_song::JukeboxSong>();
        app.register_asset_loader(jukebox_song::JukeboxSongLoader);
        app.init_asset::<instrument::Instrument>();
        app.register_asset_loader(instrument::InstrumentLoader);
        app.init_asset::<chat_type::ChatType>();
        app.register_asset_loader(chat_type::ChatTypeLoader);
        app.init_asset::<dialog::Dialog>();
        app.register_asset_loader(dialog::DialogLoader);
        app.init_asset::<timeline::Timeline>();
        app.register_asset_loader(timeline::TimelineLoader);
        app.init_asset::<test_types::TestEnvironment>();
        app.register_asset_loader(test_types::TestEnvironmentLoader);
        app.init_asset::<test_types::TestInstance>();
        app.register_asset_loader(test_types::TestInstanceLoader);
        app.add_plugins(world_clock::WorldClockPlugin);
        app.init_resource::<StaticRegistry<block::Block>>()
            .init_resource::<StaticRegistry<item::Item>>()
            .init_resource::<StaticRegistry<sound::SoundEvent>>()
            .init_resource::<StaticRegistry<entity::EntityType>>()
            .init_resource::<StaticRegistry<EnchantmentData>>()
            .init_resource::<LoadedRegistryAssets>();

        app.add_tagged_registry::<block::Block, StaticRegistry<block::Block>>(
            block_tags::ALL_BLOCK_TAGS,
        )
        .add_tagged_registry::<item::Item, StaticRegistry<item::Item>>(item_tags::ALL_ITEM_TAGS)
        .add_tagged_registry::<EnchantmentData, StaticRegistry<EnchantmentData>>(
            enchantment_tags::ALL_ENCHANTMENT_TAGS,
        )
        .add_tagged_registry::<entity::EntityType, StaticRegistry<entity::EntityType>>(
            entity_type_tags::ALL_ENTITY_TYPE_TAGS,
        )
        .add_tagged_registry::<Timeline, DynRegistryIndex<Timeline>>(&[]);

        app.init_resource::<DimensionEnvironments>();

        app.init_resource::<mcrs_core::RegistryAccess>();

        mcrs_core::snapshot_registry!(
            app,
            [
                (
                    biome::Biome,
                    "minecraft:worldgen/biome",
                    |b: &biome::Biome| mcrs_nbt::to_nbt_compound(&biome::NetworkBiome::from(b)),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    dimension::dimension_type::DimensionType,
                    "minecraft:dimension_type",
                    |d: &dimension::dimension_type::DimensionType| mcrs_nbt::to_nbt_compound(
                        &dimension::dimension_type::NetworkDimensionType::from(d)
                    ),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    timeline::Timeline,
                    "minecraft:timeline",
                    |t: &timeline::Timeline| mcrs_nbt::to_nbt_compound(
                        &timeline::NetworkTimeline::from(t)
                    ),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    chat_type::ChatType,
                    "minecraft:chat_type",
                    |v: &chat_type::ChatType| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    trim::TrimPattern,
                    "minecraft:trim_pattern",
                    |v: &trim::TrimPattern| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    trim::TrimMaterial,
                    "minecraft:trim_material",
                    |v: &trim::TrimMaterial| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::WolfVariant,
                    "minecraft:wolf_variant",
                    |v: &variant::WolfVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::WolfSoundVariant,
                    "minecraft:wolf_sound_variant",
                    |v: &variant::WolfSoundVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::PigSoundVariant,
                    "minecraft:pig_sound_variant",
                    |v: &variant::PigSoundVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::CatSoundVariant,
                    "minecraft:cat_sound_variant",
                    |v: &variant::CatSoundVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::CowSoundVariant,
                    "minecraft:cow_sound_variant",
                    |v: &variant::CowSoundVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::ChickenSoundVariant,
                    "minecraft:chicken_sound_variant",
                    |v: &variant::ChickenSoundVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::PigVariant,
                    "minecraft:pig_variant",
                    |v: &variant::PigVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::FrogVariant,
                    "minecraft:frog_variant",
                    |v: &variant::FrogVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::CatVariant,
                    "minecraft:cat_variant",
                    |v: &variant::CatVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::CowVariant,
                    "minecraft:cow_variant",
                    |v: &variant::CowVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::ChickenVariant,
                    "minecraft:chicken_variant",
                    |v: &variant::ChickenVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    variant::ZombieNautilusVariant,
                    "minecraft:zombie_nautilus_variant",
                    |v: &variant::ZombieNautilusVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    painting_variant::PaintingVariant,
                    "minecraft:painting_variant",
                    |v: &painting_variant::PaintingVariant| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    damage_type::DamageType,
                    "minecraft:damage_type",
                    |v: &damage_type::DamageType| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    banner_pattern::BannerPattern,
                    "minecraft:banner_pattern",
                    |v: &banner_pattern::BannerPattern| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    jukebox_song::JukeboxSong,
                    "minecraft:jukebox_song",
                    |v: &jukebox_song::JukeboxSong| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    instrument::Instrument,
                    "minecraft:instrument",
                    |v: &instrument::Instrument| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    dialog::Dialog,
                    "minecraft:dialog",
                    |v: &dialog::Dialog| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    test_types::TestEnvironment,
                    "minecraft:test_environment",
                    |v: &test_types::TestEnvironment| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    test_types::TestInstance,
                    "minecraft:test_instance",
                    |v: &test_types::TestInstance| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
                (
                    world_clock::WorldClock,
                    "minecraft:world_clock",
                    |v: &world_clock::WorldClock| mcrs_nbt::to_nbt_compound(v),
                    Some(mcrs_core::PackSource::vanilla_core())
                ),
            ]
        );

        app.add_systems(PostStartup, start_loading_data_pack)
            .add_systems(OnEnter(AppState::LoadingDataPack), request_data_pack_assets)
            .add_systems(
                Update,
                check_tags_ready
                    .after(TagPhase::Settled)
                    .run_if(in_state(AppState::LoadingDataPack)),
            )
            // Ordering contract: every system in this schedule that calls
            // `RegistryAccess::register` — including systems injected by the
            // `snapshot_registry!` macro elsewhere in the codebase — must
            // complete before `transition_to_playing` fires. `transition_to_playing`
            // triggers the `WorldgenFreeze → Playing` state transition, and
            // `spawn_dim_subapp` runs at `OnEnter(AppState::Playing)`, where it
            // takes the first clone of `RegistryAccess`. `RegistryAccess::register`
            // requires `Arc::get_mut` (refcount == 1); calling it after any clone
            // exists panics. The `OnEnter` schedule guarantees all its systems
            // finish before the transition completes, so the ordering holds as
            // long as no `register` call is added outside `OnEnter(WorldgenFreeze)`.
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (
                    index_timelines.before(TagPhase::Resolve),
                    (resolve_infiniburn_tags, resolve_timeline_tags).in_set(TagPhase::Resolve),
                    freeze_timelines
                        .after(TagPhase::Freeze)
                        .after(seed_world_clocks)
                        .before(transition_to_playing),
                    (
                        register_static_registries_with_access,
                        transition_to_playing,
                    )
                        .chain()
                        .after(TagPhase::Freeze),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let (definitions, report) = block::definition::load_block_definitions(&asset_server)
                .expect("the block definition corpus loads");
            tracing::info!(
                blocks = definitions.blocks().len(),
                states = report.states,
                permutations = report.permutations,
                shapes = report.shapes,
                bytes = report.table_bytes,
                elapsed = ?report.elapsed,
                "loaded block definitions"
            );
            // The hand-written `StaticRegistry<Block>` below is the older, partial
            // registry; the two coexist until it is retired.
            app.insert_resource(definitions);
        }
        {
            let mut blocks = app
                .world_mut()
                .resource_mut::<StaticRegistry<block::Block>>();
            block::minecraft::register_all_blocks(&mut blocks);
            tracing::info!(count = blocks.len(), "registered StaticRegistry<Block>");
            blocks.freeze();
            for (id, _loc, block) in blocks.iter() {
                assert_eq!(
                    id.raw() as u16,
                    block.protocol_id,
                    "block {} registered at index {} but has protocol_id {}",
                    block.identifier,
                    id.raw(),
                    block.protocol_id
                );
            }
            tracing::info!("frozen and validated StaticRegistry<Block>");
        }
        {
            let mut items = app.world_mut().resource_mut::<StaticRegistry<item::Item>>();
            item::minecraft::register_all_items(&mut items);
            tracing::info!(count = items.len(), "registered StaticRegistry<Item>");
            items.freeze();
            tracing::info!("frozen StaticRegistry<Item>");
        }
        {
            let mut sounds = app
                .world_mut()
                .resource_mut::<StaticRegistry<sound::SoundEvent>>();
            sound::minecraft::register_all_sounds(&mut sounds);
            tracing::info!(
                count = sounds.len(),
                "registered StaticRegistry<SoundEvent>"
            );
            sounds.freeze();
            tracing::info!("frozen StaticRegistry<SoundEvent>");
        }
        {
            let mut entity_types = app
                .world_mut()
                .resource_mut::<StaticRegistry<entity::EntityType>>();
            entity::minecraft::register_all_entity_types(&mut entity_types);
            tracing::info!(
                count = entity_types.len(),
                "registered StaticRegistry<EntityType>"
            );
            entity_types.freeze();
            tracing::info!("frozen StaticRegistry<EntityType>");
        }
        app.world_mut().resource_scope(
            |world, mut enchantments: Mut<StaticRegistry<EnchantmentData>>| {
                enchantment::registry::register_all_enchantments(
                    &mut enchantments,
                    world.resource::<AssetServer>(),
                );
                tracing::info!(
                    count = enchantments.len(),
                    "registered StaticRegistry<EnchantmentData>"
                );
                enchantments.freeze();
                tracing::info!("frozen StaticRegistry<EnchantmentData>");
            },
        );
    }
}

fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

// File listings baked from `assets/` at build time. Used as the fallback
// manifest when the active `AssetSource` cannot enumerate directories
// (HTTP/WASM, embedded packs without an index, etc.).
mod registry_files {
    include!(concat!(env!("OUT_DIR"), "/registry_files.rs"));
}

/// Resolve the set of `<folder>/<file>.json` paths that should be loaded
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
    fallback: &'static [&'static str],
) -> Vec<String> {
    use bevy_asset::io::AssetSourceId;
    use bevy_tasks::block_on;
    use futures_lite::StreamExt;

    let dynamic: Vec<String> = match asset_server.get_source(AssetSourceId::Default) {
        Ok(source) => {
            let reader = source.reader();
            let folder_path = std::path::Path::new(folder);
            block_on(async move {
                let mut out = Vec::new();
                if let Ok(mut stream) = reader.read_directory(folder_path).await {
                    while let Some(p) = stream.next().await {
                        if p.extension().and_then(|s| s.to_str()) != Some("json") {
                            continue;
                        }
                        if let Some(s) = p.to_str() {
                            out.push(s.to_owned());
                        }
                    }
                }
                out
            })
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
    fallback: &'static [&'static str],
) {
    let files = list_registry_files(asset_server, folder, fallback);
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

fn request_data_pack_assets(
    asset_server: Res<AssetServer>,
    mut loaded: ResMut<LoadedRegistryAssets>,
) {
    use registry_files::*;
    request_registry::<biome::Biome>(&asset_server, &mut loaded, FOLDER_BIOME, FILES_BIOME);
    request_registry::<dimension::dimension_type::DimensionType>(
        &asset_server,
        &mut loaded,
        FOLDER_DIMENSION_TYPE,
        FILES_DIMENSION_TYPE,
    );
    request_registry::<chat_type::ChatType>(
        &asset_server,
        &mut loaded,
        FOLDER_CHAT_TYPE,
        FILES_CHAT_TYPE,
    );
    request_registry::<trim::TrimPattern>(
        &asset_server,
        &mut loaded,
        FOLDER_TRIM_PATTERN,
        FILES_TRIM_PATTERN,
    );
    request_registry::<trim::TrimMaterial>(
        &asset_server,
        &mut loaded,
        FOLDER_TRIM_MATERIAL,
        FILES_TRIM_MATERIAL,
    );
    request_registry::<variant::WolfVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_WOLF_VARIANT,
        FILES_WOLF_VARIANT,
    );
    request_registry::<variant::WolfSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_WOLF_SOUND_VARIANT,
        FILES_WOLF_SOUND_VARIANT,
    );
    request_registry::<variant::PigSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_PIG_SOUND_VARIANT,
        FILES_PIG_SOUND_VARIANT,
    );
    request_registry::<variant::CatSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CAT_SOUND_VARIANT,
        FILES_CAT_SOUND_VARIANT,
    );
    request_registry::<variant::CowSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_COW_SOUND_VARIANT,
        FILES_COW_SOUND_VARIANT,
    );
    request_registry::<variant::ChickenSoundVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CHICKEN_SOUND_VARIANT,
        FILES_CHICKEN_SOUND_VARIANT,
    );
    request_registry::<variant::PigVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_PIG_VARIANT,
        FILES_PIG_VARIANT,
    );
    request_registry::<variant::FrogVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_FROG_VARIANT,
        FILES_FROG_VARIANT,
    );
    request_registry::<variant::CatVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CAT_VARIANT,
        FILES_CAT_VARIANT,
    );
    request_registry::<variant::CowVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_COW_VARIANT,
        FILES_COW_VARIANT,
    );
    request_registry::<variant::ChickenVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_CHICKEN_VARIANT,
        FILES_CHICKEN_VARIANT,
    );
    request_registry::<variant::ZombieNautilusVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_ZOMBIE_NAUTILUS_VARIANT,
        FILES_ZOMBIE_NAUTILUS_VARIANT,
    );
    request_registry::<painting_variant::PaintingVariant>(
        &asset_server,
        &mut loaded,
        FOLDER_PAINTING_VARIANT,
        FILES_PAINTING_VARIANT,
    );
    request_registry::<damage_type::DamageType>(
        &asset_server,
        &mut loaded,
        FOLDER_DAMAGE_TYPE,
        FILES_DAMAGE_TYPE,
    );
    request_registry::<banner_pattern::BannerPattern>(
        &asset_server,
        &mut loaded,
        FOLDER_BANNER_PATTERN,
        FILES_BANNER_PATTERN,
    );
    request_registry::<jukebox_song::JukeboxSong>(
        &asset_server,
        &mut loaded,
        FOLDER_JUKEBOX_SONG,
        FILES_JUKEBOX_SONG,
    );
    request_registry::<instrument::Instrument>(
        &asset_server,
        &mut loaded,
        FOLDER_INSTRUMENT,
        FILES_INSTRUMENT,
    );
    request_registry::<dialog::Dialog>(&asset_server, &mut loaded, FOLDER_DIALOG, FILES_DIALOG);
    request_registry::<timeline::Timeline>(
        &asset_server,
        &mut loaded,
        FOLDER_TIMELINE,
        FILES_TIMELINE,
    );
    request_registry::<world_clock::WorldClock>(
        &asset_server,
        &mut loaded,
        FOLDER_WORLD_CLOCK,
        FILES_WORLD_CLOCK,
    );
    request_registry::<test_types::TestEnvironment>(
        &asset_server,
        &mut loaded,
        FOLDER_TEST_ENVIRONMENT,
        FILES_TEST_ENVIRONMENT,
    );
    request_registry::<test_types::TestInstance>(
        &asset_server,
        &mut loaded,
        FOLDER_TEST_INSTANCE,
        FILES_TEST_INSTANCE,
    );
}

fn check_tags_ready(
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
fn resolve_infiniburn_tags(
    mut tags: ResMut<TagLoader<block::Block>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<block::Block>>,
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
fn index_timelines(
    timelines: Res<Assets<Timeline>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let entries: Vec<_> = timelines
        .iter()
        .filter_map(|(id, _)| rl_from_asset_path(asset_server.get_path(id)?.path()))
        .collect();
    tracing::info!(count = entries.len(), "indexed timelines");
    commands.insert_resource(DynRegistryIndex::<Timeline>::build(entries.into_iter()));
}

/// Resolve the timeline tag every dimension type names. The tag files were
/// loaded as sub-assets by `DimensionTypeLoader`, so they are available here.
fn resolve_timeline_tags(
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

fn register_static_registries_with_access(
    block_registry: Res<StaticRegistry<block::Block>>,
    item_registry: Res<StaticRegistry<item::Item>>,
    sound_registry: Res<StaticRegistry<sound::SoundEvent>>,
    entity_registry: Res<StaticRegistry<entity::EntityType>>,
    enchantment_registry: Res<StaticRegistry<EnchantmentData>>,
    mut access: ResMut<mcrs_core::RegistryAccess>,
) {
    access.register(Box::new(mcrs_core::RegistrySnapshotErased::from_static(
        "minecraft:block",
        &block_registry,
        |_, _| None,
        Some(mcrs_core::PackSource::vanilla_core()),
    )));
    access.register(Box::new(mcrs_core::RegistrySnapshotErased::from_static(
        "minecraft:item",
        &item_registry,
        |_, _| None,
        Some(mcrs_core::PackSource::vanilla_core()),
    )));
    access.register(Box::new(mcrs_core::RegistrySnapshotErased::from_static(
        "minecraft:sound_event",
        &sound_registry,
        |_, _| None,
        Some(mcrs_core::PackSource::vanilla_core()),
    )));
    access.register(Box::new(mcrs_core::RegistrySnapshotErased::from_static(
        "minecraft:entity_type",
        &entity_registry,
        |_, _| None,
        Some(mcrs_core::PackSource::vanilla_core()),
    )));
    access.register(Box::new(mcrs_core::RegistrySnapshotErased::from_static(
        "minecraft:enchantment",
        &enchantment_registry,
        |_, data| {
            use crate::enchantment::data::NetworkEnchantmentData;
            let network = NetworkEnchantmentData::from(data);
            mcrs_nbt::to_nbt_compound(&network).ok()
        },
        Some(mcrs_core::PackSource::vanilla_core()),
    )));
    tracing::info!(count = access.len(), "populated RegistryAccess");
}

pub fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
