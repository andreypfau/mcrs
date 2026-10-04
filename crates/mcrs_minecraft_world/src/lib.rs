#![allow(
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod block_transformer;
pub mod chat_type;
pub mod damage_type;
pub mod data_pack;
pub mod decorated_pot_pattern;
pub mod dialog;
pub mod dimension;
pub mod enchantment_provider;
pub mod entity;
pub mod item;
pub mod registries;
// The save on disk is native-only; the browser receives world state over the network.
#[cfg(not(target_family = "wasm"))]
pub mod save;
pub mod sulfur_cube_archetype;
pub mod test_types;
pub mod variant;
pub mod villager_trade;
pub mod worldgen;

use crate::data_pack::{
    check_tags_ready, index_biomes, index_structures, request_data_pack_assets, request_every_tag,
    resolve_infiniburn_tags, resolve_timeline_tags, start_loading_data_pack,
};
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{AssetApp, AssetServer, UntypedHandle};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::asset::JsonLoader;
use mcrs_minecraft_assets::tag::{TagPhase, TagRegistryAppExt};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_dimension::environment::{DimensionEnvironments, build_dimension_environments};
use mcrs_minecraft_entity::EntityType;
use mcrs_minecraft_environment::timeline::{NetworkTimeline, Timeline};
use mcrs_minecraft_environment::world_clock::{ClockTimeMarkers, WorldClock};
use mcrs_minecraft_item::enchantment::data::EnchantmentData;
use mcrs_minecraft_registry::DynRegistryIndex;

#[derive(Resource, Default)]
pub struct LoadedRegistryAssets {
    handles: Vec<UntypedHandle>,
}

impl LoadedRegistryAssets {
    pub fn push(&mut self, handle: UntypedHandle) {
        self.handles.push(handle);
    }

    /// True once every handle and everything it pulls in has either finished
    /// loading successfully or failed to load. The tag files a `DimensionType`
    /// names are dependencies, and a tag file's nested `#tag` references are
    /// dependencies of that, so waiting on the registry asset alone resolves
    /// those tags against a half-loaded tree. Missing or malformed files do not
    /// stall the gate; they are logged once `WorldgenFreeze` proceeds.
    ///
    /// A recursive state turns `Failed` as soon as one dependency fails, while
    /// its siblings may still be in flight, so leaf assets (templates) are
    /// requested directly and gate on their own load state.
    pub fn all_handles_settled(&self, asset_server: &AssetServer) -> bool {
        use bevy_asset::RecursiveDependencyLoadState;
        self.handles.iter().all(|h| {
            matches!(
                asset_server.recursive_dependency_load_state(h.id()),
                RecursiveDependencyLoadState::Loaded | RecursiveDependencyLoadState::Failed(_)
            )
        })
    }
}

pub struct MinecraftWorldPlugin;

impl Plugin for MinecraftWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<mcrs_minecraft_dimension::dimension_type::DimensionType>();
        app.register_asset_loader(mcrs_minecraft_dimension::dimension_type::DimensionTypeLoader);
        app.init_asset::<worldgen::world_preset::WorldPreset>();
        app.init_asset::<dimension::DimensionDefinition>();
        app.register_asset_loader(worldgen::world_preset::WorldPresetLoader);
        app.init_asset::<mcrs_minecraft_biome::Biome>();
        app.register_asset_loader(JsonLoader::<mcrs_minecraft_biome::Biome>::default());
        app.add_plugins(mcrs_minecraft_environment::world_clock::WorldClockPlugin);
        app.add_plugins(mcrs_minecraft_worldgen::bevy::WorldgenAssetsPlugin);
        app.init_resource::<LoadedRegistryAssets>();

        app.add_systems(
            OnEnter(AppState::LoadingDataPack),
            (
                request_every_tag::<mcrs_minecraft_registry::key::Block, u32>,
                request_every_tag::<mcrs_minecraft_registry::key::Fluid, u32>,
                request_every_tag::<mcrs_minecraft_item::Item, u32>,
                request_every_tag::<EnchantmentData, mcrs_minecraft_registry::Id<EnchantmentData>>,
                request_every_tag::<EntityType, mcrs_minecraft_registry::Id<EntityType>>,
                request_every_tag::<mcrs_minecraft_registry::key::Biome, u32>,
                request_every_tag::<mcrs_minecraft_registry::key::Structure, u32>,
            )
                .in_set(TagPhase::Request),
        );
        app.add_tagged_registry::<mcrs_minecraft_registry::key::Block, mcrs_minecraft_block::definition::Blocks>()
        .add_tagged_registry::<mcrs_minecraft_registry::key::Fluid, mcrs_minecraft_block::definition::Fluids>()
        .add_tagged_registry::<mcrs_minecraft_item::Item, mcrs_minecraft_item::Items>()
        .add_tagged_registry::<EnchantmentData, mcrs_minecraft_registry::Registry<EnchantmentData>>()
        .add_tagged_registry::<EntityType, mcrs_minecraft_registry::Registry<EntityType>>()
        .add_tagged_registry::<Timeline, DynRegistryIndex<Timeline>>()
        .add_tagged_registry::<mcrs_minecraft_registry::key::Biome, DynRegistryIndex<mcrs_minecraft_registry::key::Biome>>()
        .add_tagged_registry::<mcrs_minecraft_registry::key::Structure, DynRegistryIndex<mcrs_minecraft_registry::key::Structure>>();

        app.init_resource::<DimensionEnvironments>();

        app.init_resource::<mcrs_minecraft_assets::RegistryAccess>();

        {
            use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
            use mcrs_minecraft_registry::key::Block;
            use mcrs_minecraft_registry::shared::share;
            let world = app.world_mut();
            share::<mcrs_minecraft_assets::RegistryAccess>(world);
            share::<mcrs_minecraft_block::definition::Blocks>(world);
            share::<mcrs_minecraft_item::Items>(world);
            share::<DynTagRegistry<Block>>(world);
            share::<DynTagRegistry<mcrs_minecraft_item::Item>>(world);
            share::<mcrs_minecraft_assets::RegistrySnapshot<mcrs_minecraft_biome::Biome>>(world);
        }

        mcrs_minecraft_assets::snapshot_registry!(
            app,
            [
                (
                    mcrs_minecraft_biome::Biome,
                    "minecraft:worldgen/biome",
                    |b: &mcrs_minecraft_biome::Biome| mcrs_minecraft_nbt::to_nbt_tag(
                        &mcrs_minecraft_biome::NetworkBiome::from(b)
                    ),
                    Some(mcrs_minecraft_assets::PackSource::vanilla_core())
                ),
                (
                    mcrs_minecraft_dimension::dimension_type::DimensionType,
                    "minecraft:dimension_type",
                    |d: &mcrs_minecraft_dimension::dimension_type::DimensionType| {
                        mcrs_minecraft_nbt::to_nbt_tag(
                            &mcrs_minecraft_dimension::dimension_type::NetworkDimensionType::from(
                                d,
                            ),
                        )
                    },
                    Some(mcrs_minecraft_assets::PackSource::vanilla_core())
                ),
                (
                    mcrs_minecraft_worldgen::bevy::BlockStateProviderAsset,
                    "minecraft:worldgen/block_state_provider",
                    |v: &mcrs_minecraft_worldgen::bevy::BlockStateProviderAsset| {
                        mcrs_minecraft_nbt::to_nbt_tag(v)
                    },
                    Some(mcrs_minecraft_assets::PackSource::vanilla_core())
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
                    (index_biomes, index_structures).before(TagPhase::Resolve),
                    (resolve_infiniburn_tags, resolve_timeline_tags).in_set(TagPhase::Resolve),
                    build_dimension_environments
                        .after(TagPhase::Freeze)
                        .before(transition_to_playing),
                    transition_to_playing.after(TagPhase::Freeze),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let source = asset_server
                .get_source(bevy_asset::io::AssetSourceId::Default)
                .expect("default AssetSource missing");
            let path = std::path::Path::new("minecraft/version.json");
            let bytes = bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
                source.reader(),
                path,
            ))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            mcrs_minecraft_core::check_corpus_version(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        let (block_registry, item_registry) = {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let source = asset_server
                .get_source(bevy_asset::io::AssetSourceId::Default)
                .expect("default AssetSource missing");
            let path = std::path::Path::new("mcrs/reports/registries.json");
            let bytes = bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
                source.reader(),
                path,
            ))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let (statics, entity_ids) = registries::static_registries(&bytes)
                .unwrap_or_else(|report| registries::refuse(&report));
            tracing::info!(
                count = statics.tables().count(),
                "built the static registries"
            );
            let started = bevy_platform::time::Instant::now();
            let registries = registries::load_registries(&asset_server, statics)
                .unwrap_or_else(|report| registries::refuse(&report));
            tracing::info!(
                registries = registries.tables().count(),
                entries = registries.tables().map(|table| table.len()).sum::<usize>(),
                elapsed = ?started.elapsed(),
                "loaded registries"
            );
            let entity_types = registries
                .registry::<EntityType>()
                .unwrap_or_else(|| panic!("{}: no minecraft:entity_type registry", path.display()));
            let block_registry = registries
                .registry::<mcrs_minecraft_registry::key::Block>()
                .unwrap_or_else(|| panic!("{}: no minecraft:block registry", path.display()));
            let item_registry = registries
                .registry::<mcrs_minecraft_item::Item>()
                .unwrap_or_else(|| panic!("{}: no minecraft:item registry", path.display()));
            {
                use mcrs_minecraft_item::{
                    BannerPattern, InstrumentValue, JukeboxSong, PaintingVariantValue,
                    TrimMaterial, TrimPattern,
                };
                let mut access = app
                    .world_mut()
                    .resource_mut::<mcrs_minecraft_assets::RegistryAccess>();
                registries::register_loaded::<BannerPattern, _>(
                    &mut access,
                    &registries,
                    "minecraft:banner_pattern",
                    Clone::clone,
                );
                registries::register_loaded::<InstrumentValue, _>(
                    &mut access,
                    &registries,
                    "minecraft:instrument",
                    Clone::clone,
                );
                registries::register_loaded::<JukeboxSong, _>(
                    &mut access,
                    &registries,
                    "minecraft:jukebox_song",
                    Clone::clone,
                );
                registries::register_loaded::<PaintingVariantValue, _>(
                    &mut access,
                    &registries,
                    "minecraft:painting_variant",
                    Clone::clone,
                );
                registries::register_loaded::<TrimMaterial, _>(
                    &mut access,
                    &registries,
                    "minecraft:trim_material",
                    Clone::clone,
                );
                registries::register_loaded::<TrimPattern, _>(
                    &mut access,
                    &registries,
                    "minecraft:trim_pattern",
                    Clone::clone,
                );
                registries::register_loaded::<chat_type::ChatType, _>(
                    &mut access,
                    &registries,
                    "minecraft:chat_type",
                    Clone::clone,
                );
                registries::register_loaded::<test_types::TestEnvironment, _>(
                    &mut access,
                    &registries,
                    "minecraft:test_environment",
                    Clone::clone,
                );
                registries::register_loaded::<test_types::TestInstance, _>(
                    &mut access,
                    &registries,
                    "minecraft:test_instance",
                    Clone::clone,
                );
                registries::register_loaded::<dialog::Dialog, _>(
                    &mut access,
                    &registries,
                    "minecraft:dialog",
                    Clone::clone,
                );
                registries::register_loaded::<damage_type::DamageType, _>(
                    &mut access,
                    &registries,
                    "minecraft:damage_type",
                    Clone::clone,
                );
                registries::register_loaded::<block_transformer::BlockTransformer, _>(
                    &mut access,
                    &registries,
                    "minecraft:block_transformer",
                    Clone::clone,
                );
                registries::register_loaded::<EnchantmentData, _>(
                    &mut access,
                    &registries,
                    "minecraft:enchantment",
                    Clone::clone,
                );
                registries::register_loaded::<decorated_pot_pattern::DecoratedPotPattern, _>(
                    &mut access,
                    &registries,
                    "minecraft:decorated_pot_pattern",
                    Clone::clone,
                );
                registries::register_loaded::<variant::WolfVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:wolf_variant",
                    |variant| variant::NetworkWolfVariant::from(variant),
                );
                registries::register_loaded::<variant::WolfSoundVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:wolf_sound_variant",
                    Clone::clone,
                );
                registries::register_loaded::<variant::PigVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:pig_variant",
                    |variant| variant::NetworkPigVariant::from(variant),
                );
                registries::register_loaded::<variant::PigSoundVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:pig_sound_variant",
                    Clone::clone,
                );
                registries::register_loaded::<variant::CowVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:cow_variant",
                    |variant| variant::NetworkCowVariant::from(variant),
                );
                registries::register_loaded::<variant::CowSoundVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:cow_sound_variant",
                    Clone::clone,
                );
                registries::register_loaded::<variant::ChickenVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:chicken_variant",
                    |variant| variant::NetworkChickenVariant::from(variant),
                );
                registries::register_loaded::<variant::ChickenSoundVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:chicken_sound_variant",
                    Clone::clone,
                );
                registries::register_loaded::<variant::CatVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:cat_variant",
                    |variant| variant::NetworkCatVariant::from(variant),
                );
                registries::register_loaded::<variant::CatSoundVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:cat_sound_variant",
                    Clone::clone,
                );
                registries::register_loaded::<variant::FrogVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:frog_variant",
                    |variant| variant::NetworkFrogVariant::from(variant),
                );
                registries::register_loaded::<WorldClock, _>(
                    &mut access,
                    &registries,
                    "minecraft:world_clock",
                    Clone::clone,
                );
                registries::register_loaded::<Timeline, _>(
                    &mut access,
                    &registries,
                    "minecraft:timeline",
                    |timeline| NetworkTimeline::from(timeline),
                );
                registries::register_loaded::<variant::ZombieNautilusVariant, _>(
                    &mut access,
                    &registries,
                    "minecraft:zombie_nautilus_variant",
                    |variant| variant::NetworkZombieNautilusVariant::from(variant),
                );
                registries::register_loaded::<sulfur_cube_archetype::SulfurCubeArchetype, _>(
                    &mut access,
                    &registries,
                    "minecraft:sulfur_cube_archetype",
                    Clone::clone,
                );
            }
            app.insert_resource(registries.registry::<EnchantmentData>().unwrap_or_else(|| {
                panic!("{}: no minecraft:enchantment registry", path.display())
            }));
            app.insert_resource(
                registries
                    .entries::<EnchantmentData, EnchantmentData>()
                    .unwrap_or_else(|| {
                        panic!("{}: no minecraft:enchantment values", path.display())
                    }),
            );
            {
                let clocks = registries.registry::<WorldClock>().unwrap_or_else(|| {
                    panic!("{}: no minecraft:world_clock registry", path.display())
                });
                let timelines = registries
                    .column::<Timeline>("minecraft:timeline")
                    .unwrap_or_else(|| panic!("{}: no minecraft:timeline values", path.display()));
                let timeline_table = registries.table("minecraft:timeline").unwrap_or_else(|| {
                    panic!("{}: no minecraft:timeline registry", path.display())
                });
                app.insert_resource(
                    ClockTimeMarkers::derive(timelines, &clocks)
                        .expect("the load refused a time marker defined twice for one clock"),
                );
                app.insert_resource(DynRegistryIndex::<Timeline>::from_table(timeline_table));
            }
            app.insert_resource(registries);
            app.insert_resource(entity_ids);
            app.insert_resource(entity_types);
            mcrs_minecraft_registry::shared::share::<
                mcrs_minecraft_registry::Registry<EnchantmentData>,
            >(app.world_mut());
            mcrs_minecraft_registry::shared::share::<
                mcrs_minecraft_registry::Entries<EnchantmentData, EnchantmentData>,
            >(app.world_mut());
            mcrs_minecraft_registry::shared::share::<mcrs_minecraft_registry::RegistrySet>(
                app.world_mut(),
            );
            mcrs_minecraft_registry::shared::share::<entity::minecraft::EntityIds>(app.world_mut());
            (block_registry, item_registry)
        };
        {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let (definitions, report) = mcrs_minecraft_block::definition::load_block_definitions(
                &asset_server,
                &block_registry,
            )
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
            let definitions = std::sync::Arc::new(definitions);
            app.insert_resource(mcrs_minecraft_block::definition::Fluids(
                definitions.clone(),
            ));
            let items = crate::item::definitions::load_item_definitions(
                &asset_server,
                &item_registry,
                &definitions,
            )
            .expect("the item definition corpus loads");
            tracing::info!(items = items.len(), "loaded item definitions");
            app.insert_resource(mcrs_minecraft_item::Items(std::sync::Arc::new(items)));
            app.insert_resource(mcrs_minecraft_block::definition::Blocks(definitions));
        }
    }
}

pub fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
