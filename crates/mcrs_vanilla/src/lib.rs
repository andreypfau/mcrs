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
use crate::enchantment::tags as enchantment_tags;
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
            .init_resource::<StaticRegistry<entity::EntityType>>()
            .init_resource::<TagRegistry<block::Block>>()
            .init_resource::<TagRegistry<item::Item>>()
            .init_resource::<StaticRegistry<EnchantmentData>>()
            .init_resource::<TagRegistry<EnchantmentData>>()
            .add_systems(PostStartup, start_loading_data_pack)
            .add_systems(
                OnEnter(AppState::LoadingDataPack),
                (
                    request_block_tags,
                    request_item_tags,
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
            tracing::info!(count = sounds.len(), "registered StaticRegistry<SoundEvent>");
            sounds.freeze();
            tracing::info!("frozen StaticRegistry<SoundEvent>");
        }
        {
            let mut entity_types = app.world_mut().resource_mut::<StaticRegistry<entity::EntityType>>();
            entity::minecraft::register_all_entity_types(&mut entity_types);
            tracing::info!(count = entity_types.len(), "registered StaticRegistry<EntityType>");
            entity_types.freeze();
            tracing::info!("frozen StaticRegistry<EntityType>");
        }
        {
            let mut enchantments = app.world_mut().resource_mut::<StaticRegistry<EnchantmentData>>();
            enchantment::registry::register_all_enchantments(&mut enchantments);
            tracing::info!(count = enchantments.len(), "registered StaticRegistry<EnchantmentData>");
            enchantments.freeze();
            tracing::info!("frozen StaticRegistry<EnchantmentData>");
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

fn request_enchantment_tags(
    mut tags: ResMut<TagRegistry<EnchantmentData>>,
    asset_server: Res<AssetServer>,
) {
    for tag in enchantment_tags::ALL_ENCHANTMENT_TAGS {
        tags.request(tag, &asset_server);
    }
    tracing::info!(count = enchantment_tags::ALL_ENCHANTMENT_TAGS.len(), "requested enchantment tag files");
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
    enchantment_tags: Res<TagRegistry<EnchantmentData>>,
    world_preset: Res<ActiveWorldPreset>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if block_tags.all_handles_loaded(&asset_server)
        && item_tags.all_handles_loaded(&asset_server)
        && enchantment_tags.all_handles_loaded(&asset_server)
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
    mut tags: ResMut<TagRegistry<EnchantmentData>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<EnchantmentData>>,
) {
    let handles = tags.drain_handles();
    let mut resolved = 0usize;
    for (loc, handle) in handles {
        if let Some(tf) = tag_files.get(&handle) {
            let ids = TagRegistry::resolve_tag_file(tf, &tag_files, &registry);
            resolved += ids.len();
            tags.insert(loc, ids);
        } else {
            tracing::warn!("enchantment tag file not available at WorldgenFreeze: {loc}");
        }
    }
    tracing::info!(resolved_entries = resolved, "resolved TagRegistry<EnchantmentData>");
}

fn freeze_static_tags(
    mut block_tags: ResMut<TagRegistry<block::Block>>,
    mut item_tags: ResMut<TagRegistry<item::Item>>,
    mut enchantment_tags: ResMut<TagRegistry<EnchantmentData>>,
    block_registry: Res<StaticRegistry<block::Block>>,
    item_registry: Res<StaticRegistry<item::Item>>,
    enchantment_registry: Res<StaticRegistry<EnchantmentData>>,
) {
    block_tags.freeze(block_registry.len() as u32);
    item_tags.freeze(item_registry.len() as u32);
    enchantment_tags.freeze(enchantment_registry.len() as u32);
    tracing::info!("frozen TagRegistry<Block>, TagRegistry<Item>, and TagRegistry<EnchantmentData>");
}

fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
