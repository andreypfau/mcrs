use crate::biome::Biome;
use crate::dimension_type::DimensionType;
use crate::enchantment::EnchantmentData;
use crate::tag::block::TagRegistry;
use crate::version::VERSION_ID;
use crate::world::block::Block;
use crate::world::entity::player::column_view::ColumnView;
use crate::world::item::Item;
use crate::world_preset_loader::{
    DimensionTypeAsset, DimensionTypeLoader, WorldPresetAsset, WorldPresetLoader,
    resolve_preset_asset_path,
};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetApp, AssetEvent, AssetServer, Assets, Handle};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query, ResMut, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_engine::entity::player::chunk_view::PlayerChunkObserver;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{ConnectionState, InGameConnectionState, ServerSideConnection};
use mcrs_registry::Registry;
use mcrs_protocol::packets::configuration::clientbound::{
    ClientboundSelectKnownPacks, ClientboundUpdateTags,
};
use mcrs_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_protocol::packets::game::clientbound::ClientboundStartConfiguration;
use mcrs_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
use mcrs_protocol::packets::configuration::{
    ClientboundFinishConfiguration, ClientboundRegistryData,
};
use mcrs_protocol::registry::Entry;
use mcrs_protocol::resource_pack::KnownPack;
use mcrs_protocol::{Ident, WritePacket, ident, nbt};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::env;
use std::str::FromStr;
use tracing::{debug, info, warn};

/// Default world preset name used when MCRS_WORLD_PRESET is not set
const DEFAULT_WORLD_PRESET: &str = "normal";

pub(crate) struct ConfigurationStatePlugin;

impl Plugin for ConfigurationStatePlugin {
    fn build(&self, app: &mut App) {
        // Register asset types and loaders for dynamic loading
        app.init_asset::<WorldPresetAsset>()
            .init_asset::<DimensionTypeAsset>()
            .register_asset_loader(WorldPresetLoader)
            .register_asset_loader(DimensionTypeLoader);

        // Initialize resources
        app.init_resource::<LoadedWorldPreset>();
        app.init_resource::<LoadedDimensionTypes>();
        app.insert_resource(SyncedRegistries(init_synced_registries()));
        app.insert_resource(LoadedBiomes(init_biomes()));

        // Add systems
        app.add_systems(Startup, start_loading_world_preset);
        app.add_systems(Update, (process_loaded_world_preset, sync_dimension_type_changes));
        app.add_systems(bevy_app::FixedPreUpdate, on_configuration_enter);
        app.add_observer(on_configuration_ack);
        app.add_observer(on_game_configuration_ack);
    }
}

/// Resource that holds the handle to the loading world preset asset.
#[derive(Resource, Default)]
struct WorldPresetHandle(Option<Handle<WorldPresetAsset>>);

/// Start loading the world preset based on the MCRS_WORLD_PRESET environment variable.
fn start_loading_world_preset(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let preset_name = get_world_preset_name();
    let asset_path = resolve_preset_asset_path(&preset_name);

    info!(
        preset = %preset_name,
        asset_path = %asset_path,
        "Starting to load world preset via Bevy asset system"
    );

    let handle: Handle<WorldPresetAsset> = asset_server.load(asset_path);
    commands.insert_resource(WorldPresetHandle(Some(handle)));
}

/// Process the loaded world preset when it and its dependencies are ready.
fn process_loaded_world_preset(
    preset_handle: Res<WorldPresetHandle>,
    mut preset_events: MessageReader<AssetEvent<WorldPresetAsset>>,
    preset_assets: Res<Assets<WorldPresetAsset>>,
    dim_type_assets: Res<Assets<DimensionTypeAsset>>,
    mut loaded_preset: ResMut<LoadedWorldPreset>,
    mut loaded_dim_types: ResMut<LoadedDimensionTypes>,
) {
    let Some(handle) = &preset_handle.0 else {
        return;
    };

    // Check for asset events
    for event in preset_events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id } => {
                if *id != handle.id() {
                    continue;
                }

                // Get the loaded preset asset
                let Some(preset_asset) = preset_assets.get(handle) else {
                    warn!("World preset asset not found after LoadedWithDependencies event");
                    continue;
                };

                debug!(
                    preset = %preset_asset.preset_name,
                    dimension_count = preset_asset.dimensions.len(),
                    "World preset loaded with all dimension type dependencies"
                );

                // Update LoadedWorldPreset resource
                loaded_preset.preset_name = preset_asset.preset_name.clone();
                loaded_preset.dimensions = preset_asset.ordered_dimensions();
                loaded_preset.is_loaded = true;

                // Collect dimension types from the loaded assets
                let mut dim_types = Vec::new();
                for (type_ref, type_handle) in &preset_asset.dimension_type_handles {
                    if let Some(dim_type_asset) = dim_type_assets.get(type_handle) {
                        info!(
                            dimension_type = %dim_type_asset.id,
                            min_y = dim_type_asset.dimension_type.min_y,
                            height = dim_type_asset.dimension_type.height,
                            "  Loaded dimension type"
                        );
                        dim_types.push((
                            dim_type_asset.id.clone(),
                            dim_type_asset.dimension_type.clone(),
                        ));
                    } else {
                        warn!(
                            dimension_type = %type_ref,
                            "Dimension type asset not available"
                        );
                    }
                }

                // Update LoadedDimensionTypes resource
                loaded_dim_types.0 = dim_types;

                debug!(
                    preset = %preset_asset.preset_name,
                    dimensions = loaded_preset.dimensions.len(),
                    dimension_types = loaded_dim_types.0.len(),
                    "World preset configuration complete"
                );

                // Log each dimension that will be spawned
                for (dim_key, dim_type) in &loaded_preset.dimensions {
                    debug!(
                        dimension_key = %dim_key,
                        dimension_type = %dim_type,
                        "  Ready to spawn dimension"
                    );
                }
            }
            _ => {}
        }
    }
}

/// Watches for hot-reloaded dimension type assets and updates `LoadedDimensionTypes`.
/// When a player reconnects after a reload, they will receive the updated dimension types.
fn sync_dimension_type_changes(
    mut dim_type_events: MessageReader<AssetEvent<DimensionTypeAsset>>,
    dim_type_assets: Res<Assets<DimensionTypeAsset>>,
    mut loaded_dim_types: ResMut<LoadedDimensionTypes>,
    mut players: Query<(Entity, &mut ServerSideConnection), With<InGameConnectionState>>,
    mut commands: Commands,
) {
    let mut changed = false;

    for event in dim_type_events.read() {
        let id = match event {
            AssetEvent::Modified { id } => *id,
            _ => continue,
        };

        let Some(asset) = dim_type_assets.get(id) else {
            continue;
        };

        // Update the existing entry or add a new one
        if let Some(entry) = loaded_dim_types
            .0
            .iter_mut()
            .find(|(existing_id, _)| existing_id.as_str() == asset.id.as_str())
        {
            entry.1 = asset.dimension_type.clone();
            info!(
                dimension_type = %asset.id,
                "Hot-reloaded dimension type"
            );
        } else {
            loaded_dim_types
                .0
                .push((asset.id.clone(), asset.dimension_type.clone()));
            info!(
                dimension_type = %asset.id,
                "Hot-loaded new dimension type"
            );
        }

        changed = true;
    }

    // Send all connected players back to configuration to pick up the new registries
    if changed {
        for (entity, mut con) in players.iter_mut() {
            info!("Sending reconfiguration to connected player");
            con.write_packet(&ClientboundStartConfiguration);
            // Remove chunk tracking components to prevent game-state packets
            // from being sent while the client transitions to Configuration.
            // These will be re-added during the reconfiguration spawn path.
            commands
                .entity(entity)
                .remove::<ColumnView>()
                .remove::<PlayerChunkObserver>();
        }
    }
}

fn on_configuration_enter(
    mut query: Query<
        (Entity, &mut ServerSideConnection, &ConnectionState),
        Changed<ConnectionState>,
    >,
    res: Res<SyncedRegistries>,
    dimension_types: Res<LoadedDimensionTypes>,
    biomes: Res<LoadedBiomes>,
    enchantment_registry: Res<Registry<EnchantmentData>>,
    enchantment_tags: Res<TagRegistry<EnchantmentData>>,
    block_tags: Res<TagRegistry<&'static Block>>,
    block_registry: Res<Registry<&'static Block>>,
    item_tags: Res<TagRegistry<&'static Item>>,
) {
    for (entity, mut con, conn_state) in query.iter_mut() {
        if *conn_state != ConnectionState::Configuration {
            continue;
        }

        con.write_packet(&ClientboundSelectKnownPacks {
            known_packs: vec![KnownPack {
                namespace: "minecraft",
                id: "core",
                version: VERSION_ID,
            }],
        });

        let requeried_regs = vec!["variant", "damage_type"];
        for (registry_id, entries) in &res.0 {
            if !requeried_regs
                .iter()
                .any(|r| registry_id.path().contains(r))
            {
                continue;
            }

            let packet_entries = entries
                .iter()
                .map(|name| Entry {
                    id: Cow::from(name.as_str()).try_into().unwrap(),
                    data: None,
                })
                .collect::<Vec<_>>();

            let packet = ClientboundRegistryData {
                registry: Cow::from(registry_id.as_str()).try_into().unwrap(),
                entries: packet_entries,
            };
            debug!("Sending registry data: {:?}", &packet);
            con.write_packet(&packet);
        }

        // Send environment_attribute registry (keys referenced by dimension type attributes)
        {
            let mut attr_keys = Vec::new();
            for (_, dim_type) in &dimension_types.0 {
                if let Some(attrs) = &dim_type.attributes {
                    for (key, _) in &attrs.child_tags {
                        if !attr_keys.contains(key) {
                            attr_keys.push(key.clone());
                        }
                    }
                }
            }
            if !attr_keys.is_empty() {
                let entries: Vec<Entry> = attr_keys
                    .iter()
                    .map(|key| Entry {
                        id: Cow::from(key.as_str()).try_into().unwrap(),
                        data: None,
                    })
                    .collect();
                con.write_packet(&ClientboundRegistryData {
                    registry: ident!("minecraft:environment_attribute").into(),
                    entries,
                });
            }
        }

        // Send loaded dimension types to client
        let dim_entries: Vec<Entry> = dimension_types
            .0
            .iter()
            .map(|(id, dim_type)| {
                let dim_nbt = nbt::to_nbt_compound(dim_type)
                    .expect(&format!("Failed to serialize dimension type: {}", id));
                Entry {
                    id: Cow::from(id.as_str()).try_into().unwrap(),
                    data: Some(Cow::Owned(dim_nbt)),
                }
            })
            .collect();
        con.write_packet(&ClientboundRegistryData {
            registry: ident!("minecraft:dimension_type").into(),
            entries: dim_entries,
        });

        // Send loaded biomes to client
        let biome_entries: Vec<Entry> = biomes
            .0
            .iter()
            .map(|(id, biome)| {
                let biome_nbt = nbt::to_nbt_compound(biome)
                    .expect(&format!("Failed to serialize biome: {}", id));
                Entry {
                    id: Cow::from(id.as_str()).try_into().unwrap(),
                    data: Some(Cow::Owned(biome_nbt)),
                }
            })
            .collect();
        con.write_packet(&ClientboundRegistryData {
            registry: ident!("minecraft:worldgen/biome").into(),
            entries: biome_entries,
        });

        // Send enchantment registry to client
        let enchantment_entries: Vec<Entry> = enchantment_registry
            .iter_entries()
            .map(|(id, data)| {
                let enchantment_nbt = nbt::to_nbt_compound(data)
                    .expect(&format!("Failed to serialize enchantment: {}", id));
                Entry {
                    id: Cow::from(id.as_str()).try_into().unwrap(),
                    data: Some(Cow::Owned(enchantment_nbt)),
                }
            })
            .collect();
        con.write_packet(&ClientboundRegistryData {
            registry: ident!("minecraft:enchantment").into(),
            entries: enchantment_entries,
        });

        // Send tags to client
        let mut tag_registries = Vec::new();
        if !block_tags.map.is_empty() {
            tag_registries.push(
                block_tags.build_block_registry_tags(
                    ident!("minecraft:block").into(),
                    &block_registry,
                ),
            );
        }
        if !item_tags.map.is_empty() {
            tag_registries.push(
                item_tags.build_registry_tags(ident!("minecraft:item").into()),
            );
        }
        if !enchantment_tags.map.is_empty() {
            tag_registries.push(
                enchantment_tags.build_registry_tags(ident!("minecraft:enchantment").into()),
            );
        }
        debug!(
            block_tags = block_tags.map.len(),
            item_tags = item_tags.map.len(),
            enchantment_tags = enchantment_tags.map.len(),
            "Sending UpdateTags packet"
        );
        con.write_packet(&ClientboundUpdateTags {
            registries: tag_registries,
        });

        con.write_packet(&ClientboundFinishConfiguration)
    }
}

fn on_configuration_ack(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ConnectionState)>,
    mut commands: Commands,
) {
    let Ok((entity, mut state)) = query.get_mut(event.entity) else {
        return;
    };
    if *state != ConnectionState::Configuration {
        return;
    }
    let Some(_) = event.decode::<ServerboundFinishConfiguration>() else {
        return;
    };
    *state = ConnectionState::Game;
    commands.entity(entity).insert(InGameConnectionState);
}

/// Handles `ServerboundConfigurationAcknowledged` (packet 0x0F) sent during Game state.
/// This is the client's response to `ClientboundStartConfiguration` during reconfiguration.
/// Transitions the connection back to Configuration so registries can be re-sent.
fn on_game_configuration_ack(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ConnectionState)>,
    mut commands: Commands,
) {
    let Ok((entity, mut state)) = query.get_mut(event.entity) else {
        return;
    };
    if *state != ConnectionState::Game {
        return;
    }
    let Some(_) = event.decode::<ServerboundConfigurationAcknowledged>() else {
        return;
    };
    info!("Player {:?} acknowledged reconfiguration", entity);
    *state = ConnectionState::Configuration;
    commands.entity(entity).remove::<InGameConnectionState>();
}

#[derive(Default, Resource)]
struct SyncedRegistries(Vec<(Ident<String>, Vec<Ident<String>>)>);

#[derive(Default, Resource)]
pub(crate) struct LoadedDimensionTypes(pub Vec<(Ident<String>, DimensionType)>);

#[derive(Default, Resource)]
pub(crate) struct LoadedBiomes(pub Vec<(Ident<String>, Biome)>);

/// Resource containing the loaded world preset with ordered dimensions.
/// The dimensions are sorted alphabetically by dimension key for deterministic ordering.
#[derive(Resource)]
pub struct LoadedWorldPreset {
    /// The name of the loaded preset (e.g., "normal", "flat")
    pub preset_name: String,
    /// Ordered list of dimensions as (dimension_key, dimension_type_ref) tuples.
    /// For example: [("minecraft:overworld", "minecraft:overworld"), ("minecraft:the_end", "minecraft:the_end"), ...]
    pub dimensions: Vec<(Ident<String>, Ident<String>)>,
    /// Whether the preset has been fully loaded from assets
    pub is_loaded: bool,
}

impl Default for LoadedWorldPreset {
    fn default() -> Self {
        Self {
            preset_name: DEFAULT_WORLD_PRESET.to_string(),
            dimensions: Vec::new(),
            is_loaded: false,
        }
    }
}

fn init_biomes() -> Vec<(Ident<String>, Biome)> {
    let synced_registries = include_str!("../synced_registries.json");
    let json = serde_json::from_str::<Map<String, Value>>(synced_registries).unwrap();
    let biome_registry = json
        .get("worldgen/biome")
        .expect("worldgen/biome registry not found in synced_registries.json")
        .as_object()
        .expect("worldgen/biome should be an object");

    biome_registry
        .iter()
        .map(|(name, value)| {
            let biome: Biome = serde_json::from_value(value.clone())
                .expect(&format!("Failed to parse biome: {}", name));
            let id = Ident::from_str(name).unwrap();
            (id, biome)
        })
        .collect()
}

fn init_synced_registries() -> Vec<(Ident<String>, Vec<Ident<String>>)> {
    let synced_registries = include_str!("../synced_registries.json");
    let json = serde_json::from_str::<Map<String, Value>>(synced_registries).unwrap();
    json.iter()
        .map(|(registry_id, registry)| {
            let registry = registry.as_object().unwrap();
            let entries = registry
                .iter()
                .map(|(name, _value)| Ident::from_str(name).unwrap())
                .collect::<Vec<_>>();
            let registry_id: Ident<String> = Ident::from_str(registry_id).unwrap();
            (registry_id, entries)
        })
        .collect::<Vec<_>>()
}

/// Get the world preset name from the MCRS_WORLD_PRESET environment variable.
/// Returns the default 'normal' preset if not set or invalid.
/// Supports both short names ("normal") and namespaced identifiers ("minecraft:normal").
pub fn get_world_preset_name() -> String {
    match env::var("MCRS_WORLD_PRESET") {
        Ok(preset_name) => {
            let preset_name = preset_name.trim().to_lowercase();

            if preset_name.is_empty() {
                info!(
                    default_preset = DEFAULT_WORLD_PRESET,
                    "MCRS_WORLD_PRESET is empty, using default preset"
                );
                return DEFAULT_WORLD_PRESET.to_string();
            }

            // If already namespaced (contains ':'), extract the path part for validation
            let path_name = if preset_name.contains(':') {
                preset_name.split(':').last().unwrap_or(&preset_name)
            } else {
                &preset_name
            };

            info!(
                preset = %preset_name,
                "Loading world preset from MCRS_WORLD_PRESET"
            );

            // Return the original (possibly namespaced) name
            preset_name
        }
        Err(_) => {
            info!(
                default_preset = DEFAULT_WORLD_PRESET,
                "MCRS_WORLD_PRESET not set, using default preset"
            );
            DEFAULT_WORLD_PRESET.to_string()
        }
    }
}
