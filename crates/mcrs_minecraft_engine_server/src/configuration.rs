use crate::login::{SessionsById, disconnect, duplicate_login_reason};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{
    Changed, Commands, Entity, On, Query, ResMut, Resource, With, Without, resource_exists,
};
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs};
use bevy_ecs::system::Res;
use bevy_ecs::system::ScheduleSystem;
use bevy_math::{DVec3, Vec2};
use bevy_state::prelude::in_state;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_engine::bus::InboundPlayerSpawn;
use mcrs_minecraft_engine::bus::PlayerTransferSnapshot;
use mcrs_minecraft_engine::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_engine::client_info::ClientInfo;
use mcrs_minecraft_engine::dim::DimSubAppHandle;
use mcrs_minecraft_engine::dim::send_control_or_teardown;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_engine::session::HostAnchorRef;
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_level::world::world_save::WorldSave;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::identity;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
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
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::{EncodedBody, WritePacket};
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_world::SyncedRegistries;
use mcrs_minecraft_world::save::read_player_dat;
use std::sync::Arc;
use tracing::{debug, info};

use mcrs_minecraft_level::world::dimension_list::DimensionList;

/// Marker for a connection that has been sent `ClientboundSelectKnownPacks`
/// and is awaiting the client's `ServerboundSelectKnownPacks` response
/// before the rest of the Configuration data is sent.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AwaitingKnownPacks(Vec<mcrs_minecraft_datapack::KnownPack>);

pub struct ConfigurationStatePlugin;

impl Plugin for ConfigurationStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            bevy_app::FixedPreUpdate,
            start_configuration()
                .after(mcrs_minecraft_network::NetworkSet::SpawnConnections)
                .before(crate::keep_alive::handle_keepalive),
        );
        app.add_observer(on_known_packs_response);
        app.add_observer(on_configuration_ack);
        app.add_observer(on_game_configuration_ack);
        app.add_systems(
            Update,
            emit_initial_player_spawn
                .run_if(resource_exists::<DimensionList>)
                .before(mcrs_minecraft_engine::bridge::bridge_inbound_to_channel),
        );
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

#[derive(Resource)]
pub struct KnownPackOffer(pub bool);

pub fn known_pack_offer(
    offer: bool,
    registries: &Registries,
) -> Vec<mcrs_minecraft_datapack::KnownPack> {
    if offer {
        registries.known_packs().cloned().collect()
    } else {
        Vec::new()
    }
}

pub fn offered_pack(pack: &mcrs_minecraft_datapack::KnownPack) -> KnownPack<'_> {
    KnownPack {
        namespace: &pack.namespace,
        id: &pack.id,
        version: &pack.version,
    }
}

/// Whether the client may fill entries from the offered packs: only when its reply repeats the
/// offer exactly. An empty offer negotiates nothing.
pub fn negotiated(offered: &[mcrs_minecraft_datapack::KnownPack], reply: &[KnownPack]) -> bool {
    !offered.is_empty()
        && offered.len() == reply.len()
        && offered
            .iter()
            .zip(reply)
            .all(|(offered, replied)| offered_pack(offered) == *replied)
}

fn on_configuration_enter(
    mut query: Query<
        (Entity, &mut ServerSideConnection, &ConnectionState),
        (Changed<ConnectionState>, Without<AwaitingKnownPacks>),
    >,
    offer: Option<Res<KnownPackOffer>>,
    registries: Res<CurrentRegistries>,
    mut commands: Commands,
) {
    for (entity, mut con, conn_state) in query.iter_mut() {
        if *conn_state != ConnectionState::Configuration {
            continue;
        }

        con.write_packet(&ClientboundCustomPayload(Payload::Brand(Brand {
            brand: identity::BRAND,
        })));
        let offer = offer.as_deref().is_none_or(|offer| offer.0);
        let offered = known_pack_offer(offer, &registries);
        con.write_packet(&ClientboundSelectKnownPacks {
            known_packs: offered.iter().map(offered_pack).collect(),
        });
        commands.entity(entity).insert(AwaitingKnownPacks(offered));
    }
}

/// Step 2 of the Configuration handshake: triggered by
/// `ServerboundSelectKnownPacks`. Writes the registry data and update tags the load encoded, then
/// `ClientboundFinishConfiguration`. Removes the `AwaitingKnownPacks` marker so
/// the connection is eligible for future reconfiguration.
pub fn on_known_packs_response(
    event: On<Received<ServerboundSelectKnownPacks<'static>>>,
    mut query: Query<(Entity, &mut ServerSideConnection, &AwaitingKnownPacks)>,
    synced: Res<SyncedRegistries>,
    mut commands: Commands,
) {
    let Ok((entity, mut con, AwaitingKnownPacks(offered))) = query.get_mut(event.entity) else {
        return;
    };
    let Some(packs_response) = event.packet() else {
        return;
    };

    debug!(
        client_known_count = packs_response.known_packs.len(),
        "Received KnownPacks response"
    );

    let network = &synced.0;
    let known = negotiated(offered, &packs_response.known_packs);
    for body in network.registry_data(known) {
        con.write_packet(&EncodedBody::<ClientboundRegistryData>::new(body));
    }
    con.write_packet(&EncodedBody::<ClientboundUpdateTags>::new(network.tags()));

    con.write_packet(&ClientboundFinishConfiguration);

    commands.entity(entity).remove::<AwaitingKnownPacks>();
}

pub fn on_configuration_ack(
    event: On<Received<ServerboundFinishConfiguration>>,
    mut connections: Query<(
        &mut ConnectionState,
        Option<&GameProfile>,
        Option<&HostAnchorRef>,
        Option<&mut ServerSideConnection>,
    )>,
    sessions: SessionsById,
    mut commands: Commands,
) {
    let Ok((_, profile, anchor, _)) = connections.get(event.entity) else {
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
    event: On<Received<ServerboundConfigurationAcknowledged>>,
    mut query: Query<(Entity, &mut ConnectionState)>,
) {
    let Ok((entity, mut state)) = query.get_mut(event.entity) else {
        return;
    };
    info!("Player {:?} acknowledged reconfiguration", entity);
    *state = ConnectionState::Configuration;
}

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
        let saved = save
            .as_ref()
            .and_then(|save| read_player_dat(&save.0, profile.id).ok().flatten());
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
            client_info: info.cloned(),
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
