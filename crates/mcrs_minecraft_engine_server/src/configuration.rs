use crate::login::{SessionsById, disconnect, duplicate_login_reason};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::component::Component;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query, Resource, Without, resource_exists};
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs};
use bevy_ecs::system::Res;
use bevy_ecs::system::ScheduleSystem;
use bevy_math::DVec3;
use bevy_state::prelude::in_state;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_engine::SavedSpawn;
use mcrs_minecraft_engine::client_info::ClientInfo;
use mcrs_minecraft_engine::coordinator::join::place_joining_players;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::join::admit_placed_players;
use mcrs_minecraft_engine::muxer::session::HostAnchorRef;
use mcrs_minecraft_engine::persisted_registry::{
    PersistedError, PersistedRegistry, PersistedSnapshot,
};
use mcrs_minecraft_engine::to_coordinator::JoinRequested;
use mcrs_minecraft_engine::to_muxer::JoinPlaced;
use mcrs_minecraft_engine_registries::SyncedRegistries;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
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
use mcrs_minecraft_save::{RespawnData, player_dat_path, read_player_dat};
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

/// Where a joining player stands: the position and the rotation of its file, else the level
/// spawn, else a fixed point above the origin.
// chisle: no spawn search, so a missing `Pos` is the block of the level spawn as it is, possibly
// inside terrain; the search of the default position in SAVE-PLAYER-001 replaces this.
fn join_placement(
    snapshot: &PersistedSnapshot,
    spawn: Option<&RespawnData>,
) -> Result<Transform, PersistedError> {
    let pos = snapshot.pos()?;
    let rotation = snapshot.rotation()?;
    let (spawn_pos, spawn_rotation) =
        spawn.map_or((DVec3::new(0.0, 128.0, 0.0), [0.0; 2]), |spawn| {
            (
                DVec3::new(
                    f64::from(spawn.pos[0]),
                    f64::from(spawn.pos[1]),
                    f64::from(spawn.pos[2]),
                ),
                [spawn.yaw, spawn.pitch],
            )
        });
    let [yaw, pitch] = rotation.unwrap_or(spawn_rotation);
    Ok(Transform {
        translation: pos.unwrap_or(spawn_pos),
        rotation: Rotation::new(yaw, pitch),
    })
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
    con.raw.check_play_moves();

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
    saved_spawn: Option<Res<SavedSpawn>>,
    registry: Res<PersistedRegistry>,
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
        let (saved_dimension, mut snapshot) = match save.as_ref() {
            None => (None, PersistedSnapshot::default()),
            Some(save) => match read_player_dat(&save.0, profile.id) {
                Ok(None) => (None, PersistedSnapshot::default()),
                Ok(Some(dat)) => {
                    let path = player_dat_path(&save.0, profile.id);
                    let snapshot = PersistedSnapshot::from_dat(&dat, &registry, &path)
                        .unwrap_or_else(|err| {
                            tracing::error!("player data not loaded, never saving it: {err}");
                            PersistedSnapshot::unreadable()
                        });
                    (dat.dimension(), snapshot)
                }
                Err(err) => {
                    tracing::error!(uuid = %profile.id, "player data unreadable, joining empty and never saved: {err}");
                    (None, PersistedSnapshot::unreadable())
                }
            },
        };
        snapshot.insert_profile(profile);
        let spawn = saved_spawn.as_deref().map(|spawn| &spawn.0);
        let placement = match join_placement(&snapshot, spawn) {
            Ok(placement) => placement,
            Err(err) => {
                tracing::error!(uuid = %profile.id, "player pose not loaded, never saving it: {err}");
                snapshot = PersistedSnapshot::unreadable();
                snapshot.insert_profile(profile);
                join_placement(&snapshot, spawn).unwrap_or_default()
            }
        };
        snapshot.insert_transform(placement);
        if let Some(info) = info {
            snapshot.insert_client_info(info);
            snapshot.insert_view_distance(PlayerViewDistance {
                distance: info.view_distance,
                vert_distance: PlayerViewDistance::default().vert_distance,
            });
        }
        requests.write(JoinRequested {
            session: session.0,
            saved_dimension: saved_dimension.or_else(|| spawn.map(|spawn| spawn.dimension.clone())),
            snapshot,
        });
    }
}

#[cfg(test)]
mod join_placement_tests {
    use super::*;

    fn spawn() -> RespawnData {
        RespawnData {
            pos: [100, 64, -50],
            yaw: 30.0,
            pitch: -5.0,
            ..RespawnData::default()
        }
    }

    fn pose_snapshot(at: DVec3, rotation: Option<[f32; 2]>) -> PersistedSnapshot {
        let [yaw, pitch] = rotation.unwrap_or_default();
        let mut snapshot = PersistedSnapshot::default();
        snapshot.insert_transform(Transform {
            translation: at,
            rotation: Rotation::new(yaw, pitch),
        });
        if rotation.is_none() {
            snapshot.remove("Rotation");
        }
        snapshot
    }

    #[test]
    fn a_malformed_pos_fails_the_placement_instead_of_taking_the_spawn() {
        let mut snapshot = PersistedSnapshot::default();
        snapshot.put(&["Pos"], &"here").unwrap();

        assert!(join_placement(&snapshot, Some(&spawn())).is_err());
    }

    #[test]
    fn a_file_without_pos_and_rotation_joins_at_the_level_spawn() {
        let placed = join_placement(&PersistedSnapshot::default(), Some(&spawn())).unwrap();

        assert_eq!(placed.translation, DVec3::new(100.0, 64.0, -50.0));
        assert_eq!(
            [placed.rotation.yaw(), placed.rotation.pitch()],
            [30.0, -5.0]
        );
    }

    #[test]
    fn the_pos_and_the_rotation_of_a_file_come_before_the_spawn() {
        let at = DVec3::new(1.0, 2.0, 3.0);
        let placed =
            join_placement(&pose_snapshot(at, Some([90.0, 10.0])), Some(&spawn())).unwrap();

        assert_eq!(placed.translation, at);
        assert_eq!(
            [placed.rotation.yaw(), placed.rotation.pitch()],
            [90.0, 10.0]
        );
    }

    #[test]
    fn a_rotation_missing_from_a_file_with_a_pos_takes_the_spawn_rotation() {
        let at = DVec3::new(1.0, 2.0, 3.0);
        let placed = join_placement(&pose_snapshot(at, None), Some(&spawn())).unwrap();

        assert_eq!(placed.translation, at);
        assert_eq!(placed.rotation.yaw(), 30.0);
    }
}
