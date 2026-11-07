use crate::login::{SessionsById, disconnect, duplicate_login_reason};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query, Resource, Without, resource_exists};
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs};
use bevy_ecs::system::Res;
use bevy_ecs::system::ScheduleSystem;
use bevy_math::{DVec3, Vec2};
use bevy_state::prelude::in_state;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_engine::bus::PlayerTransferSnapshot;
use mcrs_minecraft_engine::client_info::ClientInfo;
use mcrs_minecraft_engine::coordinator::join::place_joining_players;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::join::admit_placed_players;
use mcrs_minecraft_engine::muxer::session::HostAnchorRef;
use mcrs_minecraft_engine::to_coordinator::JoinRequested;
use mcrs_minecraft_engine::to_muxer::JoinPlaced;
use mcrs_minecraft_engine_registries::SyncedRegistries;
use mcrs_minecraft_engine_world::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
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
use mcrs_minecraft_save::read_player_dat;
use tracing::{debug, info};

use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;

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
        app.add_message::<JoinRequested>();
        app.add_message::<JoinPlaced>();
        app.add_systems(
            Update,
            initial_player_spawn()
                .before(mcrs_minecraft_engine::muxer::bridge::bridge_inbound_to_channel),
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

/// The muxer asks where each joining player spawns, the coordinator places it, and the muxer
/// sends the spawn, in one update.
pub fn initial_player_spawn() -> ScheduleConfigs<ScheduleSystem> {
    (
        request_initial_player_spawn,
        place_joining_players.run_if(resource_exists::<DimensionList>),
        admit_placed_players,
    )
        .chain()
}

/// Runs each Update tick. For every connection in the game state whose session is still
/// unplaced, asks the coordinator where its player spawns, with the dimension that the save of
/// the player names. The muxer then sends the spawn and marks the session as joining.
///
/// While no dimension takes input yet (dims still loading), the session stays unplaced and asks
/// again next tick.
pub fn request_initial_player_spawn(
    connections: Query<(&HostAnchorRef, &ConnectionState, Option<&ClientInfo>)>,
    sessions: Query<(&Session, &SessionPlacement, &GameProfile)>,
    dim_inputs: Res<DimInputs>,
    save: Option<Res<WorldSave>>,
    mut requests: MessageWriter<JoinRequested>,
) {
    if dim_inputs.is_empty() {
        return;
    }

    for (&HostAnchorRef(host_anchor), state, info) in &connections {
        if *state != ConnectionState::Game {
            continue;
        }
        let Ok((session, placement, profile)) = sessions.get(host_anchor) else {
            continue;
        };
        if placement.place() != Place::Unplaced {
            continue;
        }
        let saved = save
            .as_ref()
            .and_then(|save| read_player_dat(&save.0, profile.id).ok().flatten());
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
        requests.write(JoinRequested {
            session: session.0,
            saved_birth: saved.as_ref().and_then(|dat| dat.birth),
            saved_dimension: saved.map(|dat| dat.dimension),
            snapshot,
        });
    }
}
