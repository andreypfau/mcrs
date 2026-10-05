use crate::WorldSave;
use crate::client_info::ClientInfo;
use crate::dim::send_control_or_teardown;
use crate::login::{GameProfile, SessionsById, disconnect, duplicate_login_reason};
use crate::world::bus::InboundPlayerSpawn;
use crate::world::bus::PlayerTransferSnapshot;
use crate::world::channel_types::{DimChannelsResource, ToDim};
use crate::world::session::HostAnchorRef;
use crate::world::sub_app_builder::DimSubAppHandle;
use bevy_app::{App, Plugin, Update};
use bevy_asset::{AssetId, AssetServer, Assets, Handle};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query, ResMut, With, Without};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs};
use bevy_ecs::system::Res;
use bevy_ecs::system::ScheduleSystem;
use bevy_math::{DVec3, Vec2};
use bevy_state::prelude::{OnEnter, in_state};
use mcrs_minecraft_assets::tag::file::{TagEntry, TagFile, TagFileSettings};
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_assets::tag::registry::TagRegistry;
use mcrs_minecraft_assets::{AppState, RegistryAccess};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, VERSION, rl};
use mcrs_minecraft_dimension::dimension_type::DimensionType;
use mcrs_minecraft_keys::{self as keys, Block, Enchantment, EntityType, Item};
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::identity;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::clientbound::Payload;
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundSelectKnownPacks, ClientboundUpdateTags, RegistryTags, TagGroup,
};
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundFinishConfiguration, ServerboundSelectKnownPacks,
};
use mcrs_minecraft_protocol::packets::configuration::{
    ClientboundCustomPayload, ClientboundFinishConfiguration, ClientboundRegistryData,
};
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
use mcrs_minecraft_protocol::registry::Entry;
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::{RegistryId, WritePacket};
use mcrs_minecraft_registry::Id;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::LoadedRegistryAssets;
use mcrs_minecraft_world::save::read_player_dat;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::world_options::{DimensionList, bake_dimensions, request_dimension_noise_settings};

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

/// Registries whose full tag set the client needs declared so item and
/// enchantment data components and dimension_type references can resolve their
/// tag pointers, paired with the `tags/<dir>` the files live under.
const DYNAMIC_TAG_REGISTRIES: &[(&str, &str)] = &[
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
];

/// The tag files of [`DYNAMIC_TAG_REGISTRIES`], kept as handles so the corpus
/// is read once by the asset system rather than re-parsed per connection.
#[derive(Resource, Default)]
pub struct DynamicRegistryTagFiles {
    per_registry: Vec<(
        &'static str,
        Vec<(ResourceLocation<Arc<str>>, Handle<TagFile>)>,
    )>,
}

fn request_dynamic_registry_tags(
    asset_server: Res<AssetServer>,
    set: Res<RegistrySet>,
    mut registry_assets: ResMut<LoadedRegistryAssets>,
    mut tag_files: ResMut<DynamicRegistryTagFiles>,
) {
    tag_files.per_registry.clear();
    let mut total = 0usize;
    for &(registry_key, tag_dir) in DYNAMIC_TAG_REGISTRIES {
        let handles: Vec<_> = mcrs_minecraft_world::data_pack::list_tag_files(&set, tag_dir)
            .into_iter()
            .map(|(location, asset_path)| {
                let handle = asset_server
                    .load_builder()
                    .with_settings(move |s: &mut TagFileSettings| {
                        s.registry_segment = tag_dir.to_string();
                    })
                    .load::<TagFile>(asset_path);
                registry_assets.push(handle.clone().untyped());
                (location, handle)
            })
            .collect();
        total += handles.len();
        tag_files.per_registry.push((registry_key, handles));
    }

    if total == 0 {
        error!(
            "no tag file was found for any dynamic registry; the client will receive no tags.              Check that the asset root holds `<namespace>/tags/`"
        );
    } else {
        info!(count = total, "requested dynamic registry tag files");
    }
}

/// Resolve one registry's tag files into the `TagGroup` list
/// `ClientboundUpdateTags` carries, flattening `#`-references through the
/// nested tag file handles the loader already resolved.
fn dynamic_tag_groups(
    entries: &[(ResourceLocation<Arc<str>>, Handle<TagFile>)],
    tag_files: &Assets<TagFile>,
    index_of: &dyn Fn(&str) -> Option<u16>,
) -> Vec<TagGroup<'static>> {
    let mut groups = Vec::with_capacity(entries.len());
    for (location, handle) in entries {
        let Some(tag_file) = tag_files.get(handle) else {
            error!(tag = %location, "tag file never loaded");
            continue;
        };
        let mut ids = Vec::new();
        let mut visited = HashSet::new();
        flatten_tag_file(tag_file, tag_files, index_of, &mut visited, &mut ids);
        ids.sort_unstable();
        ids.dedup();
        let Ok(name) = ResourceLocation::parse_cow(Cow::Owned(location.as_str().to_string()))
        else {
            continue;
        };
        groups.push(TagGroup {
            name,
            entries: ids.into_iter().map(RegistryId).collect(),
        });
    }
    groups.sort_by(|a, b| a.name.path().cmp(b.name.path()));
    groups
}

fn flatten_tag_file(
    tag_file: &TagFile,
    all_files: &Assets<TagFile>,
    index_of: &dyn Fn(&str) -> Option<u16>,
    visited: &mut HashSet<AssetId<TagFile>>,
    out: &mut Vec<u16>,
) {
    for entry in &tag_file.values {
        match entry {
            TagEntry::Element(loc) | TagEntry::OptionalElement(loc) => {
                if let Some(index) = index_of(loc.as_str()) {
                    out.push(index);
                }
            }
            TagEntry::Tag(handle) | TagEntry::OptionalTag(handle) => {
                if !visited.insert(handle.id()) {
                    continue;
                }
                if let Some(nested) = all_files.get(handle) {
                    flatten_tag_file(nested, all_files, index_of, visited, out);
                }
            }
        }
    }
}

/// Marker for a connection that has been sent `ClientboundSelectKnownPacks`
/// and is awaiting the client's `ServerboundSelectKnownPacks` response
/// before the rest of the Configuration data is sent.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AwaitingKnownPacks;

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
        app.init_resource::<DynamicRegistryTagFiles>();

        app.add_systems(bevy_app::Startup, bake_dimensions);
        app.add_systems(
            OnEnter(AppState::LoadingDataPack),
            (
                request_dimension_noise_settings,
                request_dynamic_registry_tags,
            ),
        );
        app.add_systems(bevy_app::FixedPreUpdate, start_configuration());
        app.add_observer(on_known_packs_response);
        app.add_observer(on_configuration_ack);
        app.add_observer(on_game_configuration_ack);
        // Runs in Update, before bridge_player_attach, so the spawn is sent
        // into the control channel before the same tick's dim drain.
        app.add_systems(Update, emit_initial_player_spawn);
    }
}

/// Step 1 of the Configuration handshake: detect entry into
/// `ConnectionState::Configuration`, send the brand and `ClientboundSelectKnownPacks`.
///
/// The `Without<AwaitingKnownPacks>` filter ensures the server does not
/// re-trigger the negotiation while a previous negotiation is still in
/// flight (e.g. a stray `Changed<ConnectionState>` event from a separate
/// system mutating other connection components).
///
/// Runs in `FixedPreUpdate` and reacts to `Changed<ConnectionState>`. The edge
/// is set by the `handle_login_acknowledged` observer, which fires during
/// inbound packet processing in `FixedPostUpdate`. `Changed<T>` is evaluated
/// against this system's own last-run tick, not a single frame's edge, so it
/// cannot miss the transition even when several fixed ticks elapse in one
/// main-loop frame: the change set in one tick's `FixedPostUpdate` is observed
/// at the next tick's `FixedPreUpdate`.
///
/// An embedded client connects before the registries are built, so the
/// negotiation waits for `AppState::Playing`; a skipped system keeps its
/// last-run tick, so the earlier transition is still seen as a change.
pub fn start_configuration() -> ScheduleConfigs<ScheduleSystem> {
    on_configuration_enter.run_if(in_state(AppState::Playing))
}

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

        con.write_packet(&ClientboundCustomPayload(Payload::Brand(Brand {
            brand: identity::BRAND,
        })));
        con.write_packet(&ClientboundSelectKnownPacks {
            known_packs: vec![KnownPack {
                namespace: "minecraft",
                id: "core",
                version: VERSION.id.as_str(),
            }],
        });
        commands.entity(entity).insert(AwaitingKnownPacks);
    }
}

fn tag_group(
    name: &ResourceLocation<Arc<str>>,
    entries: impl Iterator<Item = RegistryId>,
) -> TagGroup<'static> {
    TagGroup {
        name: ResourceLocation::parse_cow(Cow::Owned(name.as_str().to_string())).unwrap_or_else(
            |_| ResourceLocation::parse_cow(Cow::Borrowed("minecraft:unknown")).unwrap(),
        ),
        entries: entries.collect(),
    }
}

/// Step 2 of the Configuration handshake: triggered by
/// `ServerboundSelectKnownPacks`. Sends `ClientboundRegistryData` for the
/// 30 synced registries (alphabetical order), the `environment_attribute`
/// special case, `ClientboundUpdateTags` for the 7 tag-capable registries,
/// and finally `ClientboundFinishConfiguration`. Removes the
/// `AwaitingKnownPacks` marker so the connection is eligible for future
/// reconfiguration.
fn on_known_packs_response(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ServerSideConnection), With<AwaitingKnownPacks>>,
    access: Res<RegistryAccess>,
    set: Res<RegistrySet>,
    block_tags: Option<Res<DynTagRegistry<Block>>>,
    item_tags: Option<Res<DynTagRegistry<Item>>>,
    enchantment_tags: Option<Res<TagRegistry<Enchantment, Id<Enchantment>>>>,
    entity_type_tags: Option<Res<TagRegistry<EntityType, Id<EntityType>>>>,
    dynamic_tags: Res<DynamicRegistryTagFiles>,
    tag_files: Res<Assets<TagFile>>,
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

    // `RegistryAccess` holds exactly the registries with a network projection,
    // which is the set the game synchronizes.
    let mut registries: Vec<_> = access.iter().collect();
    registries.sort_by_key(|r| r.registry_key());

    for registry in &registries {
        let entries: Vec<Entry> = registry
            .iter_entries()
            .map(|e| {
                let pack = e
                    .pack_source
                    .as_ref()
                    .map(|ps| (ps.namespace.as_ref(), ps.id.as_ref()));
                let skip_nbt = should_skip_nbt(e.data.is_some(), pack, &client_known);

                Entry {
                    id: ResourceLocation::parse_cow(e.location.as_str()).unwrap(),
                    data: if skip_nbt {
                        None
                    } else {
                        e.data.as_ref().map(Cow::Borrowed)
                    },
                }
            })
            .collect();

        con.write_packet(&ClientboundRegistryData {
            registry: ResourceLocation::parse_cow(registry.registry_key()).unwrap(),
            entries,
        });
    }

    // environment_attribute is not a registry in RegistryAccess; it is a
    // synthetic registry built from referenced attribute keys in the
    // dimension types. The vanilla protocol still expects it to be sent.
    {
        let attr_keys: BTreeSet<&str> = set
            .column::<DimensionType>("minecraft:dimension_type")
            .unwrap_or_default()
            .iter()
            .flat_map(|dim_type| dim_type.attributes.0.keys().map(|key| key.as_str()))
            .collect();
        if !attr_keys.is_empty() {
            let entries: Vec<Entry> = attr_keys
                .iter()
                .map(|key| Entry {
                    id: ResourceLocation::parse_cow(*key).unwrap(),
                    data: None,
                })
                .collect();
            con.write_packet(&ClientboundRegistryData {
                registry: rl!("minecraft:environment_attribute").into(),
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

    if let Some(block_tags) = block_tags.as_deref().filter(|t| !t.is_empty()) {
        tag_registries.push(RegistryTags {
            registry: rl!("minecraft:block").into(),
            tags: block_tags
                .iter()
                .map(|(name, members)| tag_group(name, members.iter().map(RegistryId)))
                .collect(),
        });
    }
    if let Some(item_tags) = item_tags.as_deref().filter(|t| !t.is_empty()) {
        tag_registries.push(RegistryTags {
            registry: rl!("minecraft:item").into(),
            tags: item_tags
                .iter()
                .map(|(name, members)| tag_group(name, members.iter().map(RegistryId)))
                .collect(),
        });
    }
    if let Some(enchantment_tags) = enchantment_tags.as_deref().filter(|t| !t.is_empty()) {
        tag_registries.push(RegistryTags {
            registry: rl!("minecraft:enchantment").into(),
            tags: enchantment_tags
                .iter()
                .map(|(name, members)| tag_group(name, members.iter().map(RegistryId::from)))
                .collect(),
        });
    }
    if let Some(entity_type_tags) = entity_type_tags.as_deref().filter(|t| !t.is_empty()) {
        tag_registries.push(RegistryTags {
            registry: rl!("minecraft:entity_type").into(),
            tags: entity_type_tags
                .iter()
                .map(|(name, members)| tag_group(name, members.iter().map(RegistryId::from)))
                .collect(),
        });
    }

    // Dynamic registries that need their full tag set declared (so item /
    // enchantment data components and dimension_type references can resolve
    // tag pointers). For registries the client knows about, every referenced
    // tag must be declared even when the resolved entry list is empty.
    for (registry_key, entries) in &dynamic_tags.per_registry {
        let registry_key = *registry_key;
        let index_of = |name: &str| -> Option<u16> {
            let registry = access.iter().find(|r| r.registry_key() == registry_key)?;
            (0..=u16::MAX)
                .zip(registry.iter_entries())
                .find(|(_, e)| e.location.as_str() == name)
                .map(|(i, _)| i)
        };
        let groups = dynamic_tag_groups(entries, &tag_files, &index_of);
        if !groups.is_empty() {
            tag_registries.push(RegistryTags {
                registry: ResourceLocation::parse_cow(Cow::Owned(registry_key.to_string()))
                    .unwrap(),
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
            registry: ResourceLocation::parse_cow(Cow::Owned(reg_key.to_string())).unwrap(),
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

pub fn on_configuration_ack(
    event: On<ReceivedPacketEvent>,
    mut connections: Query<(
        &mut ConnectionState,
        Option<&GameProfile>,
        Option<&HostAnchorRef>,
        Option<&mut ServerSideConnection>,
    )>,
    sessions: SessionsById,
    mut commands: Commands,
) {
    let Ok((state, profile, anchor, _)) = connections.get(event.entity) else {
        return;
    };
    if *state != ConnectionState::Configuration {
        return;
    }
    let Some(_) = event.decode::<ServerboundFinishConfiguration>() else {
        return;
    };
    let in_play = |connection| {
        connections
            .get(connection)
            .is_ok_and(|(state, ..)| *state == ConnectionState::Game)
    };
    let duplicate = match (profile, anchor) {
        (Some(profile), Some(&HostAnchorRef(own))) => {
            sessions.in_world(profile.id, Some(own), in_play)
        }
        _ => false,
    };
    let Ok((mut state, _, _, con)) = connections.get_mut(event.entity) else {
        return;
    };
    if !duplicate {
        *state = ConnectionState::Game;
        return;
    }
    // The game switches its side to play before this check, so it refuses with a play packet.
    if let Some(mut con) = con {
        disconnect(&mut con, ConnectionState::Game, duplicate_login_reason());
    }
    commands
        .entity(event.entity)
        .remove::<ServerSideConnection>();
}

/// Handles `ServerboundConfigurationAcknowledged` sent during Game state.
/// This is the client's response to `ClientboundStartConfiguration` during reconfiguration.
/// Transitions the connection back to Configuration so registries can be re-sent.
fn on_game_configuration_ack(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ConnectionState)>,
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
}

/// What vanilla's player asks for until its client information arrives.
const VIEW_DISTANCE_FALLBACK: u8 = 2;

/// Runs each Update tick. For every connection in the game state whose session
/// is still unplaced, picks the live `DimSubAppHandle` label entity of the
/// dimension the player was saved in, or the overworld's when the save names
/// none or a dimension that is not live, sends
/// one `ToDim::Spawn` into the dimension's control channel and marks the session
/// as joining that label entity — the key used by `DimChannelsResource`, NOT a
/// sub-app-internal `Dimension` entity.
///
/// If no live label entity exists yet (dims still loading), the session stays
/// unplaced and the emit is retried next tick.
pub fn emit_initial_player_spawn(
    connections: Query<(&HostAnchorRef, &ConnectionState, Option<&ClientInfo>)>,
    mut sessions: Query<(&Session, &mut SessionPlacement, &GameProfile)>,
    live_dims: Query<(Entity, &ResourceKey<keys::Dimension>), With<DimSubAppHandle>>,
    dim_channels: Res<DimChannelsResource>,
    dimension_list: Option<Res<DimensionList>>,
    save: Option<Res<WorldSave>>,
    registries: Res<RegistrySet>,
    mut despawn_queue: ResMut<DimDespawnQueue>,
) {
    if live_dims.is_empty() {
        return;
    }

    for (&HostAnchorRef(host_anchor), state, info) in &connections {
        if *state != ConnectionState::Game {
            continue;
        }
        let Ok((session, mut placement, profile)) = sessions.get_mut(host_anchor) else {
            continue;
        };
        if placement.place() != Place::Unplaced {
            continue;
        }
        let saved = save.as_ref().and_then(|save| {
            registries
                .scope(|| read_player_dat(&save.0, profile.id))
                .ok()
                .flatten()
        });
        let live = |key: &str| {
            live_dims
                .iter()
                .find(|(_, live)| live.as_str() == key)
                .map(|(label, _)| label)
        };
        let Some(dim_label) = saved
            .as_ref()
            .and_then(|dat| live(dat.dimension.as_str()))
            .or_else(|| live(keys::dimension::OVERWORLD.as_str()))
        else {
            continue;
        };
        let Some(chan) = dim_channels.get(dim_label) else {
            continue;
        };
        let snapshot = PlayerTransferSnapshot {
            uuid: profile.id,
            username: profile.username.clone(),
            position: saved.as_ref().map_or(DVec3::new(0.0, 128.0, 0.0), |dat| {
                DVec3::from_array(dat.pos)
            }),
            rotation: saved
                .as_ref()
                .map_or(Vec2::ZERO, |dat| Vec2::from_array(dat.rotation)),
            view_distance: info
                .map(|info| info.view_distance)
                .unwrap_or(VIEW_DISTANCE_FALLBACK),
        };
        let dimensions = dimension_list
            .as_ref()
            .map_or_else(|| Arc::from([]), |list| Arc::clone(list.keys()));
        placement.set(Place::Joining(dim_label));
        send_control_or_teardown(
            &chan.control_sender,
            dim_label,
            ToDim::Spawn(InboundPlayerSpawn {
                host_anchor,
                session: session.0,
                snapshot,
                dimensions,
            }),
            &mut despawn_queue,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── dynamic registry tags ──

    /// `minecraft:always_hurts_ender_dragons` is nothing but
    /// `#minecraft:is_explosion`, so a correctly flattened group carries that
    /// tag's four members and the empty result the old working-directory read
    /// produced is distinguishable from a real one.
    #[test]
    fn dynamic_registry_tags_flatten_nested_references() {
        use bevy_asset::AssetApp;

        let mut app = App::new();
        app.add_plugins(bevy_app::TaskPoolPlugin::default());
        app.add_plugins(bevy_asset::AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        app.init_asset::<TagFile>();
        app.register_asset_loader(mcrs_minecraft_assets::tag::file::TagFileLoader);
        app.insert_resource(mcrs_minecraft_world::registries::test_registries().clone());
        app.init_resource::<LoadedRegistryAssets>();
        app.init_resource::<DynamicRegistryTagFiles>();
        app.add_systems(bevy_app::Startup, request_dynamic_registry_tags);
        app.update();

        for _ in 0..10_000 {
            let world = app.world();
            if world
                .resource::<LoadedRegistryAssets>()
                .all_handles_settled(world.resource::<AssetServer>())
            {
                break;
            }
            app.update();
        }

        let world = app.world();
        let entries = &world
            .resource::<DynamicRegistryTagFiles>()
            .per_registry
            .iter()
            .find(|(key, _)| *key == "minecraft:damage_type")
            .expect("damage_type is a dynamic tag registry")
            .1;
        assert!(
            !entries.is_empty(),
            "no damage_type tag file was discovered through the asset system"
        );

        let explosion_types = [
            "minecraft:fireworks",
            "minecraft:explosion",
            "minecraft:player_explosion",
            "minecraft:bad_respawn_point",
        ];
        let index_of = |name: &str| {
            (0..=u16::MAX)
                .zip(explosion_types)
                .find(|(_, known)| *known == name)
                .map(|(index, _)| index)
        };

        let groups = dynamic_tag_groups(entries, world.resource::<Assets<TagFile>>(), &index_of);
        assert!(!groups.is_empty(), "damage_type resolved to no tag groups");

        let flattened = groups
            .iter()
            .find(|group| group.name.as_str() == "minecraft:always_hurts_ender_dragons")
            .expect("always_hurts_ender_dragons is a shipped damage_type tag");
        assert_eq!(
            flattened.entries.iter().map(|id| id.0).collect::<Vec<_>>(),
            vec![0, 1, 2, 3],
            "the nested #minecraft:is_explosion reference was not flattened"
        );
    }

    #[test]
    fn dynamic_tag_registries_are_not_tag_capable_registries() {
        for (registry_key, _) in DYNAMIC_TAG_REGISTRIES {
            assert!(
                !TAG_CAPABLE_REGISTRIES.contains(registry_key),
                "{registry_key} is declared twice"
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
}
