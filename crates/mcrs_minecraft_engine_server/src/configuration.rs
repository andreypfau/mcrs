use crate::login::{SessionsById, disconnect, duplicate_login_reason};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query, Resource, Without, resource_exists};
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs};
use bevy_ecs::system::Res;
use bevy_ecs::system::ScheduleSystem;
use bevy_math::DVec3;
use bevy_state::prelude::in_state;
use mcrs_minecraft_assets::AppState;
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
use mcrs_minecraft_engine::{SavedSpawn, SpawnOverride};
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

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AwaitingKnownPacks(Vec<mcrs_minecraft_datapack::KnownPack>);

#[derive(Component)]
#[component(storage = "SparseSet")]
pub(crate) struct AwaitingConfigurationAck;

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

#[derive(Clone, Copy, Debug, PartialEq)]
enum JoinPose {
    At(Transform),
    Facing(Rotation),
}

fn join_placement(
    snapshot: &PersistedSnapshot,
    spawn: Option<&RespawnData>,
    spawn_override: Option<DVec3>,
) -> Result<JoinPose, PersistedError> {
    let pos = snapshot.pos()?;
    let spawn_rotation = spawn.map_or([0.0; 2], |spawn| [spawn.yaw, spawn.pitch]);
    let [yaw, pitch] = snapshot.rotation()?.unwrap_or(spawn_rotation);
    let rotation = Rotation::new(yaw, pitch);
    Ok(match spawn_override.or(pos) {
        Some(translation) => JoinPose::At(Transform {
            translation,
            rotation,
        }),
        None => JoinPose::Facing(rotation),
    })
}

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

    commands
        .entity(entity)
        .remove::<AwaitingKnownPacks>()
        .insert(AwaitingConfigurationAck);
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
    commands
        .entity(event.entity)
        .remove::<AwaitingConfigurationAck>();
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
        state.set_if_neq(ConnectionState::Game);
        return;
    }
    if let Some(mut con) = con {
        disconnect(&mut con, ConnectionState::Game, duplicate_login_reason());
    }
    commands
        .entity(event.entity)
        .remove::<ServerSideConnection>();
}

fn on_game_configuration_ack(
    event: On<Received<ServerboundConfigurationAcknowledged>>,
    mut query: Query<(Entity, &mut ConnectionState)>,
) {
    let Ok((entity, mut state)) = query.get_mut(event.entity) else {
        return;
    };
    info!("Player {:?} acknowledged reconfiguration", entity);
    state.set_if_neq(ConnectionState::Configuration);
}

pub fn initial_player_spawn() -> ScheduleConfigs<ScheduleSystem> {
    (
        request_initial_player_spawn,
        place_joining_players.run_if(resource_exists::<DimensionList>),
        admit_placed_players,
    )
        .chain()
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub fn request_initial_player_spawn(
    connections: Query<(&HostAnchorRef, &ConnectionState, Option<&ClientInfo>)>,
    sessions: Query<(&Session, &SessionPlacement, &GameProfile)>,
    dim_inputs: Res<DimInputs>,
    save: Option<Res<WorldSave>>,
    saved_spawn: Option<Res<SavedSpawn>>,
    spawn_override: Option<Res<SpawnOverride>>,
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
        let spawn_override = spawn_override.as_deref().map(|spawn| spawn.0);
        let placement = match join_placement(&snapshot, spawn, spawn_override) {
            Ok(placement) => placement,
            Err(err) => {
                tracing::error!(uuid = %profile.id, "player pose not loaded, never saving it: {err}");
                snapshot = PersistedSnapshot::unreadable();
                snapshot.insert_profile(profile);
                join_placement(&snapshot, spawn, spawn_override)
                    .unwrap_or(JoinPose::Facing(Rotation::new(0.0, 0.0)))
            }
        };
        match placement {
            JoinPose::At(transform) => snapshot.insert_transform(transform),
            JoinPose::Facing(rotation) => snapshot.insert_rotation(rotation),
        }
        if let Some(info) = info {
            snapshot.insert_client_info(info);
            snapshot.insert_view_distance(PlayerViewDistance {
                distance: info.view_distance,
                vert_distance: PlayerViewDistance::default().vert_distance,
            });
        }
        requests.write(JoinRequested {
            session: session.0,
            saved_dimension: if spawn_override.is_some() {
                None
            } else {
                saved_dimension.or_else(|| spawn.map(|spawn| spawn.dimension.clone()))
            },
            snapshot,
        });
    }
}

#[cfg(test)]
mod join_placement_tests {
    use super::*;
    use bevy_math::DVec3;

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

        assert!(join_placement(&snapshot, Some(&spawn()), None).is_err());
    }

    fn placed_at(pose: JoinPose) -> Transform {
        match pose {
            JoinPose::At(transform) => transform,
            JoinPose::Facing(rotation) => panic!("a file with a pos joins at it, got {rotation:?}"),
        }
    }

    #[test]
    fn a_file_without_pos_leaves_the_position_to_the_spawn_placement() {
        let cases = [
            (Some(spawn()), None, [30.0, -5.0]),
            (Some(spawn()), Some([90.0, 10.0]), [90.0, 10.0]),
            (None, None, [0.0, 0.0]),
        ];
        for (spawn, rotation, expected) in cases {
            let mut snapshot = PersistedSnapshot::default();
            if let Some([yaw, pitch]) = rotation {
                snapshot.insert_rotation(Rotation::new(yaw, pitch));
            }

            let placed = join_placement(&snapshot, spawn.as_ref(), None).unwrap();

            assert_eq!(
                placed,
                JoinPose::Facing(Rotation::new(expected[0], expected[1])),
                "SAVE-PLAYER-001: no position before the spawn placement, rotation {rotation:?} with spawn {:?}",
                spawn.map(|spawn| spawn.pos)
            );
        }
    }

    #[test]
    fn the_pos_and_the_rotation_of_a_file_come_before_the_spawn() {
        let at = DVec3::new(1.0, 2.0, 3.0);
        let placed = placed_at(
            join_placement(&pose_snapshot(at, Some([90.0, 10.0])), Some(&spawn()), None).unwrap(),
        );

        assert_eq!(placed.translation, at);
        assert_eq!(
            [placed.rotation.yaw(), placed.rotation.pitch()],
            [90.0, 10.0]
        );
    }

    #[test]
    fn a_rotation_missing_from_a_file_with_a_pos_takes_the_spawn_rotation() {
        let at = DVec3::new(1.0, 2.0, 3.0);
        let placed =
            placed_at(join_placement(&pose_snapshot(at, None), Some(&spawn()), None).unwrap());

        assert_eq!(placed.translation, at);
        assert_eq!(placed.rotation.yaw(), 30.0);
    }

    #[test]
    fn a_spawn_override_replaces_the_position_and_keeps_the_rotation() {
        let at = DVec3::new(1.0, 2.0, 3.0);
        let over = DVec3::new(500.0, 90.0, -40.0);
        let placed = placed_at(
            join_placement(
                &pose_snapshot(at, Some([90.0, 10.0])),
                Some(&spawn()),
                Some(over),
            )
            .unwrap(),
        );

        assert_eq!(placed.translation, over);
        assert_eq!(
            [placed.rotation.yaw(), placed.rotation.pitch()],
            [90.0, 10.0]
        );

        let fresh = placed_at(
            join_placement(&PersistedSnapshot::default(), Some(&spawn()), Some(over)).unwrap(),
        );
        assert_eq!(fresh.translation, over);
    }
}
