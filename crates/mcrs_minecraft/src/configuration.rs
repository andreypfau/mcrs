use crate::dimension_type::DimensionType;
use crate::login::GameProfile;
use crate::version::VERSION_ID;
use crate::world::bus::{InboundPlayerSpawn, PendingInboundLifecycle, PlayerTransferSnapshot};
use crate::world::entity::player::column_view::ColumnView;
use crate::world::player_index::{HostAnchorRef, PlayerIndex};
use crate::world::sub_app_builder::DimSubAppHandle;
use crate::world_preset_loader::{
    DimensionTypeAsset, DimensionTypeLoader, WorldPresetAsset, WorldPresetLoader,
    resolve_preset_asset_path,
};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetApp, AssetEvent, AssetServer, Assets, Handle};
use bevy_ecs::component::Component;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query, ResMut, With, Without};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_math::{DVec3, Vec2};
use mcrs_core::RegistryAccess;
use mcrs_core::registry::access::ErasedRegistrySnapshot;
use mcrs_core::tag::registry::TagRegistry;
use mcrs_engine::entity::player::chunk_view::PlayerChunkObserver;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{ConnectionState, InGameConnectionState, ServerSideConnection};
use mcrs_protocol::packets::configuration::clientbound::{
    ClientboundSelectKnownPacks, ClientboundUpdateTags, RegistryTags, TagGroup,
};
use mcrs_protocol::packets::configuration::serverbound::{
    ServerboundFinishConfiguration, ServerboundSelectKnownPacks,
};
use mcrs_protocol::packets::configuration::{
    ClientboundFinishConfiguration, ClientboundRegistryData,
};
use mcrs_protocol::packets::game::clientbound::ClientboundStartConfiguration;
use mcrs_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
use mcrs_protocol::registry::Entry;
use mcrs_protocol::resource_pack::KnownPack;
use mcrs_protocol::{Ident, VarInt, WritePacket, ident};
use mcrs_vanilla::block::Block as VanillaBlock;
use mcrs_vanilla::enchantment::EnchantmentData;
use mcrs_vanilla::entity::EntityType as VanillaEntityType;
use mcrs_vanilla::item::Item as VanillaItem;
use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use tracing::{debug, info, warn};

/// Default world preset name used when MCRS_WORLD_PRESET is not set
const DEFAULT_WORLD_PRESET: &str = "normal";

/// Canonical list of registries that the server synchronizes via
/// `ClientboundRegistryData` during the Configuration phase.
///
/// `RegistryAccess` holds 27 registries total (22 dynamic + 5 static), but
/// only 23 of them are protocol-synced. The 4 non-synced static registries
/// — block, item, sound_event, entity_type — remain in `RegistryAccess`
/// for internal lookups but must not be sent as `ClientboundRegistryData`.
/// Enchantment is the only static registry that is synced.
///
/// The list is sorted alphabetically so the protocol send order is
/// deterministic and reproducible across restarts.
const SYNCED_REGISTRIES: &[&str] = &[
    "minecraft:banner_pattern",
    "minecraft:cat_sound_variant",
    "minecraft:cat_variant",
    "minecraft:chat_type",
    "minecraft:chicken_sound_variant",
    "minecraft:chicken_variant",
    "minecraft:cow_sound_variant",
    "minecraft:cow_variant",
    "minecraft:damage_type",
    "minecraft:dialog",
    "minecraft:dimension_type",
    "minecraft:enchantment",
    "minecraft:frog_variant",
    "minecraft:instrument",
    "minecraft:jukebox_song",
    "minecraft:painting_variant",
    "minecraft:pig_sound_variant",
    "minecraft:pig_variant",
    "minecraft:test_environment",
    "minecraft:test_instance",
    "minecraft:timeline",
    "minecraft:trim_material",
    "minecraft:trim_pattern",
    "minecraft:wolf_sound_variant",
    "minecraft:wolf_variant",
    "minecraft:world_clock",
    "minecraft:worldgen/biome",
    "minecraft:zombie_nautilus_variant",
];

/// Allowlist of registries that emit tag groups in `ClientboundUpdateTags`.
///
/// The vanilla 1.21.11 client expects exactly these 7 entries. Any other
/// registry — even if it has tag data — is omitted from the packet.
const TAG_CAPABLE_REGISTRIES: &[&str] = &[
    "minecraft:block",
    "minecraft:enchantment",
    "minecraft:entity_type",
    "minecraft:fluid",
    "minecraft:game_event",
    "minecraft:item",
    "minecraft:worldgen/biome",
];

/// Load all `assets/minecraft/tags/<dir>/*.json` tag files, resolve each
/// referenced entry through `index_of`, and return the corresponding
/// `TagGroup` list ready to send via `ClientboundUpdateTags`. Tag references
/// (entries starting with `#`) are flattened by re-reading the referenced
/// tag file recursively; cycles are guarded by a visited set.
fn load_dynamic_registry_tags(
    tag_dir: &str,
    index_of: &dyn Fn(&str) -> Option<i32>,
) -> Vec<TagGroup<'static>> {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    let base = PathBuf::from("assets/minecraft/tags").join(tag_dir);
    let mut groups = Vec::new();

    fn walk(dir: &Path, base: &Path, files: &mut Vec<(String, PathBuf)>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, files);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let rel = match path.strip_prefix(base).ok().and_then(|p| p.to_str()) {
                Some(s) => s.trim_end_matches(".json").to_string(),
                None => continue,
            };
            files.push((rel.replace('\\', "/"), path));
        }
    }

    fn collect(
        base: &Path,
        tag_name: &str,
        index_of: &dyn Fn(&str) -> Option<i32>,
        visited: &mut HashSet<String>,
        out: &mut Vec<i32>,
    ) {
        if !visited.insert(tag_name.to_string()) {
            return;
        }
        let rel = tag_name.strip_prefix("minecraft:").unwrap_or(tag_name);
        let path = base.join(format!("{rel}.json"));
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let parsed: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return,
        };
        let values = match parsed.get("values").and_then(|v| v.as_array()) {
            Some(v) => v,
            None => return,
        };
        for v in values {
            let s = match v.as_str() {
                Some(s) => s,
                None => continue,
            };
            if let Some(rest) = s.strip_prefix('#') {
                collect(base, rest, index_of, visited, out);
            } else if let Some(idx) = index_of(s) {
                out.push(idx);
            }
        }
    }

    let mut files = Vec::new();
    walk(&base, &base, &mut files);
    for (rel, _path) in files {
        let tag_name = format!("minecraft:{rel}");
        let mut visited = HashSet::new();
        let mut ids = Vec::new();
        collect(&base, &tag_name, index_of, &mut visited, &mut ids);
        ids.sort();
        ids.dedup();
        groups.push(TagGroup {
            name: Ident::new(Cow::Owned(tag_name)).unwrap(),
            entries: ids.into_iter().map(VarInt).collect(),
        });
    }
    groups.sort_by(|a, b| a.name.path().cmp(b.name.path()));
    groups
}

/// Marker for a connection that has been sent `ClientboundSelectKnownPacks`
/// and is awaiting the client's `ServerboundSelectKnownPacks` response
/// before the rest of the Configuration data is sent.
#[derive(Component)]
#[component(storage = "SparseSet")]
struct AwaitingKnownPacks;

/// True iff the entry's NBT body should be omitted from `ClientboundRegistryData`
/// because the client already has the pack that sourced it.
///
/// The check has two guards: the entry must actually carry data (keys-only
/// registries like `block`/`item` always send `data: None`), and the entry's
/// pack source must match one the client confirmed in its known-packs list.
fn should_skip_nbt(
    entry_has_data: bool,
    pack_source: Option<(&str, &str)>,
    client_known: &HashSet<(&str, &str)>,
) -> bool {
    entry_has_data
        && pack_source
            .map(|ps| client_known.contains(&ps))
            .unwrap_or(false)
}

pub struct ConfigurationStatePlugin;

impl Plugin for ConfigurationStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<WorldPresetAsset>()
            .init_asset::<DimensionTypeAsset>()
            .register_asset_loader(WorldPresetLoader)
            .register_asset_loader(DimensionTypeLoader);

        app.init_resource::<LoadedWorldPreset>();
        app.init_resource::<LoadedDimensionTypes>();

        app.add_systems(Startup, start_loading_world_preset);
        app.add_systems(Update, (process_loaded_world_preset, sync_dimension_type_changes));
        app.add_systems(bevy_app::FixedPreUpdate, on_configuration_enter);
        app.add_observer(on_known_packs_response);
        app.add_observer(on_configuration_ack);
        app.add_observer(on_game_configuration_ack);
        // Runs in Update, before bridge_player_attach, so the spawn is buffered
        // in PendingInboundLifecycle before the same tick's extract.
        app.add_systems(Update, emit_initial_player_spawn);
    }
}

#[derive(Resource, Default)]
struct WorldPresetHandle(Option<Handle<WorldPresetAsset>>);

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

    for event in preset_events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            if *id != handle.id() {
                continue;
            }

            let Some(preset_asset) = preset_assets.get(handle) else {
                warn!("World preset asset not found after LoadedWithDependencies event");
                continue;
            };

            debug!(
                preset = %preset_asset.preset_name,
                dimension_count = preset_asset.dimensions.len(),
                "World preset loaded with all dimension type dependencies"
            );

            loaded_preset.preset_name = preset_asset.preset_name.clone();
            loaded_preset.dimensions = preset_asset.ordered_dimensions();
            loaded_preset.is_loaded = true;

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

            loaded_dim_types.0 = dim_types;

            debug!(
                preset = %preset_asset.preset_name,
                dimensions = loaded_preset.dimensions.len(),
                dimension_types = loaded_dim_types.0.len(),
                "World preset configuration complete"
            );

            for (dim_key, dim_type) in &loaded_preset.dimensions {
                debug!(
                    dimension_key = %dim_key,
                    dimension_type = %dim_type,
                    "  Ready to spawn dimension"
                );
            }
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

    if changed {
        for (entity, mut con) in players.iter_mut() {
            info!("Sending reconfiguration to connected player");
            con.write_packet(&ClientboundStartConfiguration);
            commands
                .entity(entity)
                .remove::<ColumnView>()
                .remove::<PlayerChunkObserver>();
        }
    }
}

/// Step 1 of the Configuration handshake: detect entry into
/// `ConnectionState::Configuration` and send `ClientboundSelectKnownPacks`.
///
/// The `Without<AwaitingKnownPacks>` filter ensures the server does not
/// re-trigger the negotiation while a previous negotiation is still in
/// flight (e.g. a stray `Changed<ConnectionState>` event from a separate
/// system mutating other connection components).
fn on_configuration_enter(
    mut query: Query<
        (Entity, &mut ServerSideConnection, &ConnectionState),
        (Changed<ConnectionState>, Without<AwaitingKnownPacks>),
    >,
    mut commands: Commands,
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
        commands.entity(entity).insert(AwaitingKnownPacks);
    }
}

/// Step 2 of the Configuration handshake: triggered by
/// `ServerboundSelectKnownPacks`. Sends `ClientboundRegistryData` for the
/// 23 synced registries (alphabetical order), the `environment_attribute`
/// special case, `ClientboundUpdateTags` for the 7 tag-capable registries,
/// and finally `ClientboundFinishConfiguration`. Removes the
/// `AwaitingKnownPacks` marker so the connection is eligible for future
/// reconfiguration.
fn on_known_packs_response(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ServerSideConnection), With<AwaitingKnownPacks>>,
    access: Res<RegistryAccess>,
    dimension_types: Res<LoadedDimensionTypes>,
    block_tags: Res<TagRegistry<VanillaBlock>>,
    item_tags: Res<TagRegistry<VanillaItem>>,
    enchantment_tags: Res<TagRegistry<EnchantmentData>>,
    entity_type_tags: Res<TagRegistry<VanillaEntityType>>,
    mut commands: Commands,
) {
    let Ok((entity, mut con)) = query.get_mut(event.entity) else {
        return;
    };
    let Some(packs_response) = event.decode::<ServerboundSelectKnownPacks>() else {
        return;
    };

    let client_known: HashSet<(&str, &str)> = packs_response
        .known_packs
        .iter()
        .map(|p| (p.namespace, p.id))
        .collect();

    debug!(
        client_known_count = client_known.len(),
        "Received KnownPacks response"
    );

    // RegistryData: filter to the 23 protocol-synced registries and send
    // them in alphabetical order by registry key for deterministic output.
    let mut registries: Vec<&dyn ErasedRegistrySnapshot> = access
        .iter()
        .filter(|r| SYNCED_REGISTRIES.contains(&r.registry_key()))
        .collect();
    registries.sort_by_key(|r| r.registry_key());

    for registry in &registries {
        let entries: Vec<Entry> = registry
            .iter_entries()
            .map(|e| {
                let pack = e
                    .pack_source
                    .map(|ps| (ps.namespace.as_ref(), ps.id.as_ref()));
                let skip_nbt = should_skip_nbt(e.data.is_some(), pack, &client_known);

                Entry {
                    id: Cow::from(e.location.as_str()).try_into().unwrap(),
                    data: if skip_nbt {
                        None
                    } else {
                        e.data.map(Cow::Borrowed)
                    },
                }
            })
            .collect();

        con.write_packet(&ClientboundRegistryData {
            registry: Cow::from(registry.registry_key()).try_into().unwrap(),
            entries,
        });
    }

    // environment_attribute is not a registry in RegistryAccess; it is a
    // synthetic registry built from referenced attribute keys in the
    // dimension types. The vanilla protocol still expects it to be sent.
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

    // UpdateTags: explicit allowlist of tag-capable registries. Static tags
    // (block, item, enchantment) come from TagRegistry<T>; the remaining
    // four tag-capable registries (entity_type, fluid, game_event,
    // worldgen/biome) have no DynTagRegistry resource registered, so empty
    // groups are sent so the vanilla client does not warn about missing
    // registries.
    let mut tag_registries = Vec::new();

    if !block_tags.is_empty() {
        let groups: Vec<TagGroup> = block_tags
            .iter()
            .map(|(tag_loc, bitset)| TagGroup {
                name: Ident::new(Cow::Owned(tag_loc.as_str().to_string()))
                    .unwrap_or_else(|_| Ident::new(Cow::Borrowed("minecraft:unknown")).unwrap()),
                entries: bitset.iter().map(|id| VarInt(id.raw() as i32)).collect(),
            })
            .collect();
        tag_registries.push(RegistryTags {
            registry: ident!("minecraft:block").into(),
            tags: groups,
        });
    }
    if !item_tags.is_empty() {
        let groups: Vec<TagGroup> = item_tags
            .iter()
            .map(|(tag_loc, bitset)| TagGroup {
                name: Ident::new(Cow::Owned(tag_loc.as_str().to_string()))
                    .unwrap_or_else(|_| Ident::new(Cow::Borrowed("minecraft:unknown")).unwrap()),
                entries: bitset.iter().map(|id| VarInt(id.raw() as i32)).collect(),
            })
            .collect();
        tag_registries.push(RegistryTags {
            registry: ident!("minecraft:item").into(),
            tags: groups,
        });
    }
    if !enchantment_tags.is_empty() {
        let groups: Vec<TagGroup> = enchantment_tags
            .iter()
            .map(|(tag_loc, bitset)| TagGroup {
                name: Ident::new(Cow::Owned(tag_loc.as_str().to_string()))
                    .unwrap_or_else(|_| Ident::new(Cow::Borrowed("minecraft:unknown")).unwrap()),
                entries: bitset.iter().map(|id| VarInt(id.raw() as i32)).collect(),
            })
            .collect();
        tag_registries.push(RegistryTags {
            registry: ident!("minecraft:enchantment").into(),
            tags: groups,
        });
    }
    if !entity_type_tags.is_empty() {
        let groups: Vec<TagGroup> = entity_type_tags
            .iter()
            .map(|(tag_loc, bitset)| TagGroup {
                name: Ident::new(Cow::Owned(tag_loc.as_str().to_string()))
                    .unwrap_or_else(|_| Ident::new(Cow::Borrowed("minecraft:unknown")).unwrap()),
                entries: bitset.iter().map(|id| VarInt(id.raw() as i32)).collect(),
            })
            .collect();
        tag_registries.push(RegistryTags {
            registry: ident!("minecraft:entity_type").into(),
            tags: groups,
        });
    }

    // Dynamic registries that need their full tag set declared (so item /
    // enchantment data components and dimension_type references can resolve
    // tag pointers). For registries the client knows about, every referenced
    // tag must be declared even when the resolved entry list is empty.
    for (registry_key, tag_dir) in [
        ("minecraft:damage_type", "damage_type"),
        ("minecraft:dialog", "dialog"),
        ("minecraft:timeline", "timeline"),
        ("minecraft:banner_pattern", "banner_pattern"),
        ("minecraft:instrument", "instrument"),
        ("minecraft:painting_variant", "painting_variant"),
        ("minecraft:cat_variant", "cat_variant"),
        ("minecraft:wolf_variant", "wolf_variant"),
        ("minecraft:trim_material", "trim_material"),
        ("minecraft:trim_pattern", "trim_pattern"),
        ("minecraft:jukebox_song", "jukebox_song"),
    ] {
        let index_of = |name: &str| -> Option<i32> {
            access
                .iter()
                .find(|r| r.registry_key() == registry_key)?
                .iter_entries()
                .enumerate()
                .find(|(_, e)| e.location.as_str() == name)
                .map(|(i, _)| i as i32)
        };
        let groups = load_dynamic_registry_tags(tag_dir, &index_of);
        if !groups.is_empty() {
            tag_registries.push(RegistryTags {
                registry: Ident::new(Cow::Owned(registry_key.to_string())).unwrap(),
                tags: groups,
            });
        }
    }

    for &reg_key in TAG_CAPABLE_REGISTRIES {
        if reg_key == "minecraft:block"
            || reg_key == "minecraft:item"
            || reg_key == "minecraft:enchantment"
            || reg_key == "minecraft:entity_type"
        {
            continue;
        }
        tag_registries.push(RegistryTags {
            registry: Ident::new(Cow::Owned(reg_key.to_string())).unwrap(),
            tags: vec![],
        });
    }

    debug!(
        registry_count = registries.len(),
        tag_registry_count = tag_registries.len(),
        "Sending Configuration data"
    );

    con.write_packet(&ClientboundUpdateTags {
        registries: tag_registries,
    });

    con.write_packet(&ClientboundFinishConfiguration);

    commands.entity(entity).remove::<AwaitingKnownPacks>();
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

/// Runs each Update tick. For every connection in `InGameConnectionState` whose
/// host-anchor still has `current_dim == Entity::PLACEHOLDER` (initial join not yet
/// emitted), picks the first live `DimSubAppHandle` label entity (keyed by insertion
/// order, which is consistent within a tick) and buffers one `InboundPlayerSpawn`
/// into `PendingInboundLifecycle.per_dim[dim_label].spawns`, then sets
/// `PlayerLocation.current_dim` to that label.
///
/// `current_dim` is set to the DimSubAppHandle LABEL entity (the key used by
/// `PendingInboundLifecycle` and the extract closure), NOT a sub-app-internal
/// `Dimension` entity. The two live in different worlds and must not be confused.
///
/// If no live label entity exists yet (dims still loading), the emit is deferred:
/// no spawn is pushed and `current_dim` stays `PLACEHOLDER`. The idempotent guard
/// (`current_dim != PLACEHOLDER`) ensures at most one initial-join spawn per player.
pub fn emit_initial_player_spawn(
    connections: Query<&HostAnchorRef, With<InGameConnectionState>>,
    mut player_index: ResMut<PlayerIndex>,
    live_dims: Query<Entity, With<DimSubAppHandle>>,
    profiles: Query<&GameProfile>,
    mut lifecycle: ResMut<PendingInboundLifecycle>,
) {
    let dim_label = match live_dims.iter().next() {
        Some(e) => e,
        None => return,
    };

    for anchor_ref in connections.iter() {
        let host_anchor = anchor_ref.0;
        let Some(location) = player_index.get_mut(&host_anchor) else {
            continue;
        };
        if location.current_dim != Entity::PLACEHOLDER {
            continue;
        }
        let Ok(profile) = profiles.get(host_anchor) else {
            continue;
        };
        let snapshot = PlayerTransferSnapshot {
            uuid: profile.id,
            username: profile.username.clone(),
            position: DVec3::new(0.0, 64.0, 0.0),
            rotation: Vec2::ZERO,
        };
        location.current_dim = dim_label;
        lifecycle
            .per_dim
            .entry(dim_label)
            .or_default()
            .spawns
            .push(InboundPlayerSpawn { host_anchor, snapshot });
    }
}

#[derive(Default, Resource)]
pub(crate) struct LoadedDimensionTypes(pub Vec<(Ident<String>, DimensionType)>);

/// Resource containing the loaded world preset with ordered dimensions.
/// The dimensions are sorted alphabetically by dimension key for deterministic ordering.
#[derive(Resource)]
pub struct LoadedWorldPreset {
    pub preset_name: String,
    pub dimensions: Vec<(Ident<String>, Ident<String>)>,
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

            let _path_name = if preset_name.contains(':') {
                preset_name.split(':').next_back().unwrap_or(&preset_name)
            } else {
                &preset_name
            };

            info!(
                preset = %preset_name,
                "Loading world preset from MCRS_WORLD_PRESET"
            );

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── SYNCED_REGISTRIES ──

    #[test]
    fn synced_registries_count() {
        assert_eq!(SYNCED_REGISTRIES.len(), 28);
    }

    #[test]
    fn synced_registries_excludes_non_synced() {
        assert!(!SYNCED_REGISTRIES.contains(&"minecraft:block"));
        assert!(!SYNCED_REGISTRIES.contains(&"minecraft:item"));
        assert!(!SYNCED_REGISTRIES.contains(&"minecraft:sound_event"));
        assert!(!SYNCED_REGISTRIES.contains(&"minecraft:entity_type"));
    }

    #[test]
    fn synced_registries_includes_enchantment() {
        assert!(SYNCED_REGISTRIES.contains(&"minecraft:enchantment"));
    }

    #[test]
    fn synced_registries_includes_worldgen_biome() {
        assert!(SYNCED_REGISTRIES.contains(&"minecraft:worldgen/biome"));
    }

    #[test]
    fn synced_registries_includes_dimension_type() {
        assert!(SYNCED_REGISTRIES.contains(&"minecraft:dimension_type"));
    }

    #[test]
    fn synced_registries_is_sorted() {
        let mut sorted = SYNCED_REGISTRIES.to_vec();
        sorted.sort();
        assert_eq!(sorted, SYNCED_REGISTRIES);
    }

    // ── TAG_CAPABLE_REGISTRIES ──

    #[test]
    fn tag_capable_registries_count() {
        assert_eq!(TAG_CAPABLE_REGISTRIES.len(), 7);
    }

    #[test]
    fn tag_capable_registries_is_sorted() {
        let mut sorted = TAG_CAPABLE_REGISTRIES.to_vec();
        sorted.sort();
        assert_eq!(sorted, TAG_CAPABLE_REGISTRIES);
    }

    #[test]
    fn tag_capable_registries_contents() {
        for expected in &[
            "minecraft:block",
            "minecraft:enchantment",
            "minecraft:entity_type",
            "minecraft:fluid",
            "minecraft:game_event",
            "minecraft:item",
            "minecraft:worldgen/biome",
        ] {
            assert!(
                TAG_CAPABLE_REGISTRIES.contains(expected),
                "missing {expected}"
            );
        }
    }

    // ── should_skip_nbt: KnownPacks NBT-skip logic ──

    #[test]
    fn skip_nbt_when_pack_known_and_data_present() {
        let mut known = HashSet::new();
        known.insert(("minecraft", "core"));
        assert!(should_skip_nbt(true, Some(("minecraft", "core")), &known));
    }

    #[test]
    fn no_skip_nbt_when_data_is_none() {
        let mut known = HashSet::new();
        known.insert(("minecraft", "core"));
        assert!(!should_skip_nbt(false, Some(("minecraft", "core")), &known));
    }

    #[test]
    fn no_skip_nbt_when_pack_not_known() {
        let known: HashSet<(&str, &str)> = HashSet::new();
        assert!(!should_skip_nbt(true, Some(("minecraft", "core")), &known));
    }

    #[test]
    fn no_skip_nbt_when_no_pack_source() {
        let mut known = HashSet::new();
        known.insert(("minecraft", "core"));
        assert!(!should_skip_nbt(true, None, &known));
    }

    #[test]
    fn no_skip_nbt_when_pack_namespace_differs() {
        let mut known = HashSet::new();
        known.insert(("minecraft", "core"));
        assert!(!should_skip_nbt(true, Some(("modid", "core")), &known));
    }

    #[test]
    fn no_skip_nbt_when_pack_id_differs() {
        let mut known = HashSet::new();
        known.insert(("minecraft", "core"));
        assert!(!should_skip_nbt(true, Some(("minecraft", "extra")), &known));
    }

    // ── filter behavior ──

    #[test]
    fn filtering_excludes_non_synced_static_registries() {
        let candidates = [
            "minecraft:block",
            "minecraft:item",
            "minecraft:sound_event",
            "minecraft:entity_type",
            "minecraft:enchantment",
            "minecraft:worldgen/biome",
        ];
        let filtered: Vec<&&str> = candidates
            .iter()
            .filter(|k| SYNCED_REGISTRIES.contains(*k))
            .collect();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&&"minecraft:enchantment"));
        assert!(filtered.contains(&&"minecraft:worldgen/biome"));
    }
}
