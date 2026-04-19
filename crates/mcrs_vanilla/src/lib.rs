#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod block;
pub mod enchantment;
pub mod entity;
pub mod explosion;
pub mod item;
pub mod material;
pub mod player_action;
pub mod sound;
pub mod biome;
pub mod dimension;
pub mod value;
pub mod worldgen;
pub mod variant;
pub mod trim;
pub mod damage_type;
pub mod painting_variant;
pub mod banner_pattern;
pub mod jukebox_song;
pub mod instrument;

use crate::block::tags as block_tags;
use crate::enchantment::data::EnchantmentData;
use crate::enchantment::registry::{LoadedEnchantments, VANILLA_ENCHANTMENTS};
use crate::enchantment::tags as enchantment_tags;
use crate::enchantment::tags::EnchantmentTags;
use crate::enchantment::EnchantmentDataLoader;
use crate::item::tags as item_tags;
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{AssetApp, AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_core::tag::file::TagFile;
use mcrs_core::tag::key::TaggedRegistry;
use mcrs_core::{AppState, ResourceLocation, StaticRegistry, TagRegistry};
use crate::dimension::dimension_type::DimensionType;
use crate::worldgen::world_preset::ActiveWorldPreset;

pub struct MinecraftCorePlugin;

impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<dimension::dimension_type::DimensionType>();
        app.register_asset_loader(dimension::dimension_type::DimensionTypeLoader);
        app.init_asset::<biome::Biome>();
        app.register_asset_loader(biome::BiomeLoader);
        app.init_asset::<worldgen::noise_settings::NoiseGeneratorSettings>();
        app.init_asset::<worldgen::structure_set::StructureSet>();
        app.init_asset::<dimension::level_stem::DimensionDefinition>();
        app.init_asset::<worldgen::world_preset::WorldPreset>();
        app.register_asset_loader(worldgen::world_preset::WorldPresetLoader);
        app.init_asset::<variant::WolfVariant>();
        app.register_asset_loader(variant::WolfVariantLoader);
        app.init_asset::<variant::WolfSoundVariant>();
        app.register_asset_loader(variant::WolfSoundVariantLoader);
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
        app.init_resource::<StaticRegistry<block::Block>>()
            .init_resource::<StaticRegistry<item::Item>>()
            .init_resource::<StaticRegistry<sound::SoundEvent>>()
            .init_resource::<TagRegistry<block::Block>>()
            .init_resource::<TagRegistry<item::Item>>()
            .init_asset::<EnchantmentData>()
            .register_asset_loader(EnchantmentDataLoader)
            .init_resource::<EnchantmentTags>()
            .add_systems(PostStartup, start_loading_data_pack)
            .add_systems(
                OnEnter(AppState::LoadingDataPack),
                (
                    request_block_tags,
                    request_item_tags,
                    request_enchantment_assets,
                    request_enchantment_tags,
                    request_world_preset,
                ),
            )
            .add_systems(
                Update,
                check_tags_ready.run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (
                    resolve_block_tags,
                    resolve_infiniburn_tags,
                    resolve_item_tags,
                    resolve_enchantment_tags,
                    freeze_static_tags,
                    transition_to_playing,
                )
                    .chain(),
            );
    }

    fn finish(&self, app: &mut App) {
        {
            let mut blocks = app
                .world_mut()
                .resource_mut::<StaticRegistry<block::Block>>();
            block::minecraft::register_all_blocks(&mut blocks);
            tracing::info!(count = blocks.len(), "registered StaticRegistry<Block>");
        }
        {
            let mut items = app.world_mut().resource_mut::<StaticRegistry<item::Item>>();
            item::minecraft::register_all_items(&mut items);
            tracing::info!(count = items.len(), "registered StaticRegistry<Item>");
        }
        {
            let mut sounds = app
                .world_mut()
                .resource_mut::<StaticRegistry<sound::SoundEvent>>();
            sound::minecraft::register_all_sounds(&mut sounds);
            tracing::info!(count = sounds.len(), "registered StaticRegistry<SoundEvent>");
        }
    }
}

fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

fn request_block_tags(mut tags: ResMut<TagRegistry<block::Block>>, asset_server: Res<AssetServer>) {
    for tag in block_tags::ALL_BLOCK_TAGS {
        tags.request(tag, &asset_server);
    }
}

fn request_item_tags(mut tags: ResMut<TagRegistry<item::Item>>, asset_server: Res<AssetServer>) {
    for tag in item_tags::ALL_ITEM_TAGS {
        tags.request(tag, &asset_server);
    }
}

fn request_enchantment_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut loaded = LoadedEnchantments::new();
    for &name in VANILLA_ENCHANTMENTS {
        let loc = ResourceLocation::parse(name).expect("invalid enchantment RL");
        let path = format!("{}/enchantment/{}.json", loc.namespace(), loc.path());
        let handle = asset_server.load::<EnchantmentData>(path);
        loaded.push(loc, handle);
    }
    tracing::info!(count = loaded.len(), "requested enchantment assets");
    commands.insert_resource(loaded);
}

fn request_enchantment_tags(asset_server: Res<AssetServer>) {
    // Enchantment tag files are loaded as dependencies of the enchantment data
    // assets themselves. We just log the count for symmetry.
    tracing::info!(
        count = enchantment_tags::ALL_ENCHANTMENT_TAGS.len(),
        "enchantment tag keys registered"
    );
}

fn request_world_preset(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load::<worldgen::world_preset::WorldPreset>(
        worldgen::world_preset::DEFAULT_WORLD_PRESET,
    );
    tracing::info!("requested world preset: {}", worldgen::world_preset::DEFAULT_WORLD_PRESET);
    commands.insert_resource(ActiveWorldPreset { handle });
}

fn check_tags_ready(
    block_tags: Res<TagRegistry<block::Block>>,
    item_tags: Res<TagRegistry<item::Item>>,
    world_preset: Res<ActiveWorldPreset>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if block_tags.all_handles_loaded(&asset_server)
        && item_tags.all_handles_loaded(&asset_server)
        && asset_server.is_loaded_with_dependencies(&world_preset.handle)
    {
        tracing::info!("all tag files and world preset loaded — entering WorldgenFreeze");
        next.set(AppState::WorldgenFreeze);
    }
}

fn resolve_block_tags(
    mut tags: ResMut<TagRegistry<block::Block>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<block::Block>>,
) {
    let handles = tags.drain_handles();
    let mut resolved = 0usize;
    for (loc, handle) in handles {
        if let Some(tf) = tag_files.get(&handle) {
            let ids = TagRegistry::resolve_tag_file(tf, &tag_files, &registry);
            resolved += ids.len();
            tags.insert(loc, ids);
        } else {
            tracing::warn!("block tag file not available at WorldgenFreeze: {loc}");
        }
    }
    tracing::info!(resolved_entries = resolved, "resolved TagRegistry<Block>");
}

/// Resolve infiniburn tag files from loaded `DimensionType` assets into
/// `TagRegistry<Block>`. The tag files were loaded as sub-assets by
/// `DimensionTypeLoader`, so they're guaranteed to be available here.
fn resolve_infiniburn_tags(
    mut tags: ResMut<TagRegistry<block::Block>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<block::Block>>,
    dim_types: Res<Assets<DimensionType>>,
) {
    let mut resolved = 0usize;
    for (_id, dim_type) in dim_types.iter() {
        let key = dim_type.infiniburn.key();
        if let Some(tf) = tag_files.get(dim_type.infiniburn.handle()) {
            let ids = TagRegistry::resolve_tag_file(tf, &tag_files, &registry);
            resolved += ids.len();
            tags.insert(key.location().clone(), ids);
        } else {
            tracing::warn!("infiniburn tag file not available at WorldgenFreeze: {}", key.as_str());
        }
    }
    if resolved > 0 {
        tracing::info!(resolved_entries = resolved, "resolved infiniburn tags");
    }
}

fn resolve_item_tags(
    mut tags: ResMut<TagRegistry<item::Item>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<item::Item>>,
) {
    let handles = tags.drain_handles();
    let mut resolved = 0usize;
    for (loc, handle) in handles {
        if let Some(tf) = tag_files.get(&handle) {
            let ids = TagRegistry::resolve_tag_file(tf, &tag_files, &registry);
            resolved += ids.len();
            tags.insert(loc, ids);
        } else {
            tracing::warn!("item tag file not available at WorldgenFreeze: {loc}");
        }
    }
    tracing::info!(resolved_entries = resolved, "resolved TagRegistry<Item>");
}

fn resolve_enchantment_tags(
    mut tags: ResMut<EnchantmentTags>,
    tag_files: Res<Assets<TagFile>>,
    loaded: Res<LoadedEnchantments>,
    enchantment_assets: Res<Assets<EnchantmentData>>,
    asset_server: Res<AssetServer>,
) {
    let mut resolved = 0usize;
    for tag_key in enchantment_tags::ALL_ENCHANTMENT_TAGS {
        let segment = EnchantmentData::REGISTRY_PATH.to_string();
        let handle = asset_server
            .load_with_settings::<TagFile, mcrs_core::tag::file::TagFileSettings>(
                tag_key.asset_path(),
                move |s| {
                    s.registry_segment = segment.clone();
                },
            );
        if let Some(tf) = tag_files.get(&handle) {
            let ids =
                EnchantmentTags::resolve_tag_file(tf, &tag_files, &loaded, &enchantment_assets);
            resolved += ids.len();
            tags.insert(tag_key.to_arc().location().clone(), ids);
        } else {
            tracing::warn!(
                "enchantment tag file not available at WorldgenFreeze: {}",
                tag_key.as_str()
            );
        }
    }
    tracing::info!(
        resolved_entries = resolved,
        "resolved EnchantmentTags"
    );
}

fn freeze_static_tags(
    mut block_tags: ResMut<TagRegistry<block::Block>>,
    mut item_tags: ResMut<TagRegistry<item::Item>>,
    block_registry: Res<StaticRegistry<block::Block>>,
    item_registry: Res<StaticRegistry<item::Item>>,
) {
    block_tags.freeze(block_registry.len() as u32);
    item_tags.freeze(item_registry.len() as u32);
    tracing::info!("frozen TagRegistry<Block> and TagRegistry<Item>");
}

fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
