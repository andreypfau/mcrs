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
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query, ResMut, With, Without};
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs};
use bevy_ecs::system::Res;
use bevy_ecs::system::ScheduleSystem;
use bevy_math::{DVec3, Vec2};
use bevy_state::prelude::{OnEnter, in_state};
use mcrs_minecraft_assets::{AppState, RegistryAccess};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, VERSION};
use mcrs_minecraft_dimension::dimension_type::DimensionType;
use mcrs_minecraft_keys::{
    self as keys, BannerPattern, Block, CatVariant, DamageType, Dialog, Enchantment, EntityType,
    Instrument, Item, JukeboxSong, PaintingVariant, Timeline, TrimMaterial, TrimPattern,
    WolfVariant,
};
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::identity;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::clientbound::Payload;
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundSelectKnownPacks, ClientboundUpdateTags, RegistryTags,
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
use mcrs_minecraft_protocol::tags::tags_payload;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::save::read_player_dat;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, info};

use crate::world_options::{DimensionList, bake_dimensions, request_dimension_noise_settings};

/// Registries the client expects in `ClientboundUpdateTags` whose tags the server does not send.
const EMPTY_TAG_REGISTRIES: [ResourceLocation<&str>; 3] =
    [keys::Fluid::KEY, keys::GameEvent::KEY, keys::Biome::KEY];

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
        app.add_systems(bevy_app::Startup, bake_dimensions);
        app.add_systems(
            OnEnter(AppState::LoadingDataPack),
            request_dimension_noise_settings,
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

fn tags_of<R: RegistryKey>(set: &RegistrySet) -> Option<RegistryTags<'static>> {
    let payload = tags_payload(&set.tags::<R>()?);
    (!payload.tags.is_empty()).then_some(payload)
}

pub fn update_tags(set: &RegistrySet) -> ClientboundUpdateTags<'static> {
    let registries = [
        tags_of::<Block>(set),
        tags_of::<Item>(set),
        tags_of::<Enchantment>(set),
        tags_of::<EntityType>(set),
        tags_of::<DamageType>(set),
        tags_of::<Dialog>(set),
        tags_of::<Timeline>(set),
        tags_of::<BannerPattern>(set),
        tags_of::<Instrument>(set),
        tags_of::<PaintingVariant>(set),
        tags_of::<CatVariant>(set),
        tags_of::<WolfVariant>(set),
        tags_of::<TrimMaterial>(set),
        tags_of::<TrimPattern>(set),
        tags_of::<JukeboxSong>(set),
    ]
    .into_iter()
    .flatten()
    .chain(EMPTY_TAG_REGISTRIES.map(|registry| RegistryTags {
        registry: registry.into(),
        tags: Vec::new(),
    }))
    .collect();
    ClientboundUpdateTags { registries }
}

/// Step 2 of the Configuration handshake: triggered by
/// `ServerboundSelectKnownPacks`. Sends `ClientboundRegistryData` for the
/// 30 synced registries (alphabetical order), the `environment_attribute`
/// special case, `ClientboundUpdateTags` from the set's tags,
/// and finally `ClientboundFinishConfiguration`. Removes the
/// `AwaitingKnownPacks` marker so the connection is eligible for future
/// reconfiguration.
fn on_known_packs_response(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ServerSideConnection), With<AwaitingKnownPacks>>,
    access: Res<RegistryAccess>,
    set: Res<RegistrySet>,
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
                    id: ResourceLocation::read_cow(e.location.as_str()).unwrap(),
                    data: if skip_nbt {
                        None
                    } else {
                        e.data.as_ref().map(Cow::Borrowed)
                    },
                }
            })
            .collect();

        con.write_packet(&ClientboundRegistryData {
            registry: ResourceLocation::read_cow(registry.registry_key()).unwrap(),
            entries,
        });
    }

    // environment_attribute is not a registry in RegistryAccess; it is a
    // synthetic registry built from referenced attribute keys in the
    // dimension types. The vanilla protocol still expects it to be sent.
    {
        let attr_keys: BTreeSet<&str> = set
            .column::<DimensionType>(keys::DimensionType::KEY.as_str())
            .unwrap_or_default()
            .iter()
            .flat_map(|dim_type| dim_type.attributes.0.keys().map(|key| key.as_str()))
            .collect();
        if !attr_keys.is_empty() {
            let entries: Vec<Entry> = attr_keys
                .iter()
                .map(|key| Entry {
                    id: ResourceLocation::read_cow(*key).unwrap(),
                    data: None,
                })
                .collect();
            con.write_packet(&ClientboundRegistryData {
                registry: keys::EnvironmentAttribute::KEY.into(),
                entries,
            });
        }
    }

    let update_tags = update_tags(&set);
    debug!(
        registry_count = registries.len(),
        tag_registry_count = update_tags.registries.len(),
        "Sending Configuration data"
    );
    con.write_packet(&update_tags);

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

    // ── should_skip_nbt: KnownPacks NBT-skip logic ──

    #[test]
    fn skip_nbt_when_pack_known_and_data_present() {
        let mut known = HashSet::new();
        known.insert(("minecraft", "core"));
        assert!(should_skip_nbt(true, Some(("minecraft", "core")), &known));
    }
}
