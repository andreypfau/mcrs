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
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::VANILLA_PACK;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, VERSION};
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::identity;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::clientbound::Payload;
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundSelectKnownPacks, ClientboundUpdateTags,
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
use mcrs_minecraft_protocol::tags::tags_payload_of;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::save::read_player_dat;
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, info};

use crate::world_options::{DimensionList, bake_dimensions, request_dimension_noise_settings};

/// Marker for a connection that has been sent `ClientboundSelectKnownPacks`
/// and is awaiting the client's `ServerboundSelectKnownPacks` response
/// before the rest of the Configuration data is sent.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AwaitingKnownPacks;

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

pub fn update_tags(set: &RegistrySet) -> ClientboundUpdateTags<'static> {
    let mut statics: Vec<&str> = set
        .tables()
        .map(|table| table.registry().as_str())
        .filter(|registry| !set.is_world_registry(registry))
        .collect();
    statics.sort_unstable();
    let registries = set
        .synced()
        .map(|(table, _)| table.registry().as_str())
        .chain(statics)
        .filter_map(|registry| set.tag_table(registry))
        .filter(|table| !table.is_empty())
        .map(|table| tags_payload_of(table))
        .collect();
    ClientboundUpdateTags { registries }
}

pub fn registry_data<'a>(
    set: &'a RegistrySet,
    client_known: &HashSet<(&str, &str)>,
) -> Vec<ClientboundRegistryData<'a>> {
    let knows_vanilla = client_known.contains(&("minecraft", "core"));
    set.synced()
        .map(|(table, column)| {
            let registry = table.registry().as_str();
            let entries = table
                .names()
                .iter()
                .zip(column)
                .enumerate()
                .map(|(id, (name, network))| {
                    let from_vanilla = set.pack_of(registry, id) == Some(VANILLA_PACK);
                    Entry {
                        id: ResourceLocation::read_cow(name.as_str()).unwrap(),
                        data: (!(knows_vanilla && from_vanilla))
                            .then_some(Cow::Borrowed(&network.0)),
                    }
                })
                .collect();
            ClientboundRegistryData {
                registry: ResourceLocation::read_cow(registry).unwrap(),
                entries,
            }
        })
        .collect()
}

/// Step 2 of the Configuration handshake: triggered by
/// `ServerboundSelectKnownPacks`. Sends `ClientboundRegistryData` for every
/// synced registry, `ClientboundUpdateTags` from the set's tags, and finally
/// `ClientboundFinishConfiguration`. Removes the `AwaitingKnownPacks` marker so
/// the connection is eligible for future reconfiguration.
fn on_known_packs_response(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ServerSideConnection), With<AwaitingKnownPacks>>,
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

    let registries = registry_data(&set, &client_known);
    for packet in &registries {
        con.write_packet(packet);
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
/// dimension the player was saved in, or the first dimension of the list when
/// the save names none or a dimension that is not live, sends
/// one `ToDim::Spawn` into the dimension's control channel and marks the session
/// as joining that label entity — the key used by `DimChannelsResource`, NOT a
/// sub-app-internal `Dimension` entity.
///
/// If no live label entity exists yet (dims still loading), the session stays
/// unplaced and the emit is retried next tick.
pub fn emit_initial_player_spawn(
    connections: Query<(&HostAnchorRef, &ConnectionState, Option<&ClientInfo>)>,
    mut sessions: Query<(&Session, &mut SessionPlacement, &GameProfile)>,
    live_dims: Query<
        (Entity, &ResourceKey<mcrs_minecraft_dimension::Dimension>),
        With<DimSubAppHandle>,
    >,
    dim_channels: Res<DimChannelsResource>,
    dimension_list: Res<DimensionList>,
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
        let live = |key: &ResourceKey<mcrs_minecraft_dimension::Dimension>| {
            live_dims
                .iter()
                .find(|(_, live)| *live == key)
                .map(|(label, _)| label)
        };
        let Some(dim_label) = saved
            .as_ref()
            .and_then(|dat| live(&dat.dimension))
            .or_else(|| live(dimension_list.keys().first()?))
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
        let dimensions = Arc::clone(dimension_list.keys());
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
