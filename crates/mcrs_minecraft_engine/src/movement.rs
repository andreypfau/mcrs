use crate::bus::to;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::prelude::{
    Changed, DetectChangesMut, Entity, Message, MessageReader, MessageWriter, Mut, On, Query, Res,
    With,
};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_math::DVec3;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot, ServerboundMovePlayerStatusOnly, ServerboundMoveVehicle,
};

use crate::bus::{OutboundPlayerPacket, PacketPayload};
use crate::client_on_ground::ClientOnGround;
use crate::coordinator::clock::GameTime;
use crate::teleport::{
    AcceptOutcome, MAX_XZ, MAX_Y, TeleportState, landing, position_packet, teleport_to,
};
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_physics::{Relative, Teleport};
use mcrs_minecraft_text::Text;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_move_pos);
        app.add_observer(handle_move_pos_rot);
        app.add_observer(handle_move_rot);
        app.add_observer(handle_move_status_only);
        app.add_observer(handle_move_vehicle);
        app.add_observer(handle_accept_teleportation);
        app.add_message::<PlayerMovement>();
        if let Err(error) = app.try_register_required_components::<Player, ClientOnGround>() {
            tracing::error!("a player does not require its on-ground flag: {error}");
        }
        app.add_systems(FixedUpdate, process_movement.in_set(TickSet::ClientInput));
        app.add_systems(
            FixedPostUpdate,
            teleport.before(VoxelUpdateSet::NetworkSync),
        );
    }
}

fn handle_move_pos(
    on: On<Received<ServerboundMovePlayerPos>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if let Some(p) = on.packet() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: Some(p.position.into()),
            look: None,
            on_ground: Some(ClientOnGround(p.flags.on_ground())),
        });
    }
}

fn handle_move_pos_rot(
    on: On<Received<ServerboundMovePlayerPosRot>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if let Some(p) = on.packet() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: Some(p.position.into()),
            look: Some(Rotation::new(p.look.yaw, p.look.pitch)),
            on_ground: Some(ClientOnGround(p.flags.on_ground())),
        });
    }
}

fn handle_move_rot(
    on: On<Received<ServerboundMovePlayerRot>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if let Some(p) = on.packet() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: None,
            look: Some(Rotation::new(p.look.yaw, p.look.pitch)),
            on_ground: Some(ClientOnGround(p.flags.on_ground())),
        });
    }
}

fn handle_move_status_only(
    on: On<Received<ServerboundMovePlayerStatusOnly>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if let Some(p) = on.packet() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: None,
            look: None,
            on_ground: Some(ClientOnGround(p.flags.on_ground())),
        });
    }
}

/// The server has no vehicles, so a vehicle move moves nothing; it still counts as a move for the
/// resend of a pending teleport. Its on-ground flag is the vehicle's, not the player's.
fn handle_move_vehicle(
    on: On<Received<ServerboundMoveVehicle>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if on.packet().is_some() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: None,
            look: None,
            on_ground: None,
        });
    }
}

#[derive(Message)]
pub struct PlayerMovement {
    entity: Entity,
    position: Option<DVec3>,
    look: Option<Rotation>,
    on_ground: Option<ClientOnGround>,
}

pub(crate) fn process_movement(
    mut reader: MessageReader<PlayerMovement>,
    mut query: Query<(
        &Owner,
        Mut<TeleportState>,
        Mut<Transform>,
        Mut<ClientOnGround>,
    )>,
    time: Res<GameTime>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for movement in reader.read() {
        let Ok((owner, mut state, mut transform, mut on_ground)) = query.get_mut(movement.entity)
        else {
            continue;
        };
        if state.pending().is_some() {
            if let Some(destination) = state.resend_due(*time) {
                let id = state.sent(destination, *time);
                packets.write(resend(owner, id, destination, transform.rotation));
            }
            continue;
        }
        if let Some(position) = movement.position {
            transform.set_if_neq(transform.with_translation(reachable(position)));
        }
        if let Some(look) = movement.look {
            transform.set_if_neq(transform.with_rotation(look));
        }
        if let Some(reported) = movement.on_ground {
            on_ground.set_if_neq(reported);
        }
        state.sync(*transform);
    }
}

fn reachable(position: DVec3) -> DVec3 {
    const MAX_POS: DVec3 = DVec3::new(MAX_XZ, MAX_Y, MAX_XZ);
    position.clamp(-MAX_POS, MAX_POS)
}

/// The resend of a pending teleport: absolute values, with the position of `destination`, the
/// rotation of the player and zero velocity.
fn resend(
    owner: &Owner,
    teleport_id: i32,
    destination: Transform,
    rotation: Rotation,
) -> OutboundPlayerPacket {
    let teleport = Teleport {
        position: destination.translation,
        velocity: DVec3::ZERO,
        yaw: rotation.yaw(),
        pitch: rotation.pitch(),
        relative: Relative::empty(),
    };
    position(owner, teleport_id, &teleport)
}

/// The correction of an accept: the position of `destination`, and the client keeps its rotation
/// and velocity.
fn correction(owner: &Owner, teleport_id: i32, destination: Transform) -> OutboundPlayerPacket {
    position(owner, teleport_id, &teleport_to(destination, destination))
}

fn position(owner: &Owner, teleport_id: i32, teleport: &Teleport) -> OutboundPlayerPacket {
    to(
        owner.0,
        PacketPayload::PlayerPosition(position_packet(teleport_id, teleport)),
    )
}

fn teleport(
    mut clients: Query<(&Owner, &mut TeleportState, &Transform), (Changed<Transform>, With<Owner>)>,
    time: Res<GameTime>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, mut state, transform) in &mut clients {
        let known = state.synced_transform();
        if *transform == known {
            continue;
        }
        let teleport = teleport_to(known, *transform);
        let destination = landing(known, &teleport);
        let teleport_id = state.sent(destination, *time);
        packet_writer.write(to(
            owner.0,
            PacketPayload::PlayerPosition(position_packet(teleport_id, &teleport)),
        ));
    }
}

/// An accept of the pending teleport at its destination confirms it, and an accept at another
/// position gets a correction: the position of an accept never becomes the position of the
/// player. An accept of the newest id once it is confirmed disconnects the client.
fn handle_accept_teleportation(
    event: On<Received<ServerboundAcceptTeleportation>>,
    mut players: Query<(&Owner, &mut TeleportState)>,
    time: Res<GameTime>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(accept) = event.packet() else {
        return;
    };
    let Ok((owner, mut state)) = players.get_mut(event.entity) else {
        return;
    };
    match state.accept(accept.teleport_id.0, accept.position.into()) {
        AcceptOutcome::Mismatch { destination } => {
            let id = state.sent(destination, *time);
            packets.write(correction(owner, id, destination));
        }
        AcceptOutcome::Invalid => {
            packets.write(to(
                owner.0,
                PacketPayload::Disconnect(Text::translate(
                    "multiplayer.disconnect.invalid_player_movement",
                    vec![],
                )),
            ));
        }
        AcceptOutcome::Confirmed | AcceptOutcome::Ignored => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::PacketTarget;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_minecraft_engine_world::session::PlayerSession;

    #[test]
    fn a_server_move_sends_one_position_with_the_next_teleport_id_and_a_synced_one_none() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerPacket>>();
        world.init_resource::<GameTime>();
        let moved_session = PlayerSession(1);
        let moved = world
            .spawn((
                Owner(moved_session),
                Transform::default().with_translation(DVec3::new(1.0, 0.0, 0.0)),
                TeleportState::default(),
            ))
            .id();
        let synced = world
            .spawn((
                Owner(PlayerSession(2)),
                Transform::default(),
                TeleportState::default(),
            ))
            .id();

        world.run_system_once(teleport).unwrap();

        let sent: Vec<_> = world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect();
        assert_eq!(sent.len(), 1, "only the moved player is told");
        assert!(
            matches!(sent[0].target, PacketTarget::SinglePlayer(session) if session == moved_session)
        );
        let PacketPayload::PlayerPosition(position) = &sent[0].data else {
            panic!("a move is sent as a player position");
        };
        assert_eq!(position.teleport_id.0, 1, "TP-CONFIRM-ID-001");
        assert_eq!(position.position, DVec3::new(1.0, 0.0, 0.0));

        let pending = |entity| {
            world
                .get::<TeleportState>(entity)
                .unwrap()
                .pending()
                .map(|p| p.id)
        };
        assert_eq!((pending(moved), pending(synced)), (Some(1), None));
    }

    fn movement(entity: Entity, position: DVec3) -> PlayerMovement {
        PlayerMovement {
            entity,
            position: Some(position),
            look: None,
            on_ground: None,
        }
    }

    fn movement_world() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<PlayerMovement>>();
        world.init_resource::<Messages<OutboundPlayerPacket>>();
        world.init_resource::<GameTime>();
        world
    }

    fn sent_ids(world: &mut World) -> Vec<i32> {
        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .filter_map(|packet| match packet.data {
                PacketPayload::PlayerPosition(position) => Some(position.teleport_id.0),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_client_move_applies_only_once_the_teleport_is_confirmed() {
        let mut world = movement_world();
        let mut unconfirmed = TeleportState::default();
        unconfirmed.sent(Transform::default(), GameTime(0));
        let awaiting = world
            .spawn((
                Owner(PlayerSession(1)),
                Transform::default(),
                unconfirmed,
                ClientOnGround::default(),
            ))
            .id();
        let confirmed = world
            .spawn((
                Owner(PlayerSession(2)),
                Transform::default(),
                TeleportState::default(),
                ClientOnGround::default(),
            ))
            .id();
        let to = DVec3::new(5.0, 64.0, 5.0);
        for entity in [awaiting, confirmed] {
            world.write_message(movement(entity, to));
        }

        world.run_system_once(process_movement).unwrap();

        let at = |entity| world.get::<Transform>(entity).unwrap().translation;
        assert_eq!((at(awaiting), at(confirmed)), (DVec3::ZERO, to));
        assert_eq!(
            sent_ids(&mut world),
            [] as [i32; 0],
            "the teleport is fresh"
        );
    }

    #[test]
    fn an_accept_away_from_the_destination_gets_a_correction_and_never_moves_the_player() {
        use mcrs_minecraft_core::Look;
        use mcrs_minecraft_protocol::Encode;

        let destination = Transform::default().with_translation(DVec3::new(3.0, 64.0, 0.0));
        let mut world = movement_world();
        world.add_observer(handle_accept_teleportation);
        let mut state = TeleportState::default();
        let id = state.sent(destination, GameTime(0));
        let player = world
            .spawn((Owner(PlayerSession(1)), destination, state))
            .id();
        let mut body = Vec::new();
        ServerboundAcceptTeleportation {
            teleport_id: id.into(),
            position: DVec3::new(8.0, 64.0, 0.0).into(),
            look: Look {
                yaw: 90.0,
                pitch: 10.0,
            },
        }
        .encode(&mut body)
        .unwrap();

        world.trigger(Received::<ServerboundAcceptTeleportation>::new(
            player,
            body.into(),
        ));

        assert_eq!(
            world.get::<Transform>(player),
            Some(&destination),
            "MVC-LAT-008"
        );
        let pending = world.get::<TeleportState>(player).unwrap().pending();
        assert!(pending.is_some());
        assert_eq!(sent_ids(&mut world).len(), 1);
    }

    #[test]
    fn a_vehicle_move_after_21_ticks_resends_the_pending_teleport() {
        use mcrs_minecraft_protocol::Encode;

        let mut world = movement_world();
        world.add_observer(handle_move_vehicle);
        let destination = Transform::default().with_translation(DVec3::new(3.0, 64.0, 0.0));
        let mut state = TeleportState::default();
        state.sent(destination, GameTime(0));
        let player = world
            .spawn((
                Owner(PlayerSession(1)),
                destination,
                state,
                ClientOnGround::default(),
            ))
            .id();
        let mut body = Vec::new();
        ServerboundMoveVehicle {
            position: DVec3::new(9.0, 64.0, 0.0).into(),
            look: Default::default(),
            on_ground: false,
        }
        .encode(&mut body)
        .unwrap();
        let resent_at = |world: &mut World, tick| {
            world.insert_resource(GameTime(tick));
            world.trigger(Received::<ServerboundMoveVehicle>::new(
                player,
                body.clone().into(),
            ));
            world.run_system_once(process_movement).unwrap();
            sent_ids(world)
        };

        assert_eq!(
            resent_at(&mut world, 20),
            [] as [i32; 0],
            "TP-CONFIRM-RES-006"
        );
        assert_eq!(
            resent_at(&mut world, 21),
            [2],
            "TP-CONFIRM-RES-006, TP-CONFIRM-ID-002"
        );
        let state = world.get::<TeleportState>(player).unwrap();
        assert_eq!(
            state.pending().map(|pending| pending.destination),
            Some(destination),
            "TP-CONFIRM-RES-004"
        );
        assert_eq!(world.get::<Transform>(player), Some(&destination));
    }

    #[test]
    fn the_on_ground_flag_follows_each_move_packet_of_the_player_and_no_vehicle_move() {
        use mcrs_minecraft_protocol::Encode;
        use mcrs_minecraft_protocol::packets::table::Row;
        use mcrs_minecraft_protocol_game::MoveFlags;

        fn flag_after<P: Row + Encode>(world: &mut World, player: Entity, packet: &P) -> bool {
            let mut body = Vec::new();
            packet.encode(&mut body).unwrap();
            world.trigger(Received::<P>::new(player, body.into()));
            world.run_system_once(process_movement).unwrap();
            world.get::<ClientOnGround>(player).unwrap().0
        }
        let flags = |on_ground| MoveFlags::new().with_on_ground(on_ground);
        let vehicle = |on_ground| ServerboundMoveVehicle {
            position: Default::default(),
            look: Default::default(),
            on_ground,
        };

        let mut world = movement_world();
        world.add_observer(handle_move_pos);
        world.add_observer(handle_move_pos_rot);
        world.add_observer(handle_move_rot);
        world.add_observer(handle_move_status_only);
        world.add_observer(handle_move_vehicle);
        let player = world
            .spawn((
                Owner(PlayerSession(1)),
                Transform::default(),
                TeleportState::default(),
                ClientOnGround::default(),
            ))
            .id();

        let reported = [
            flag_after(
                &mut world,
                player,
                &ServerboundMovePlayerStatusOnly { flags: flags(true) },
            ),
            flag_after(&mut world, player, &vehicle(false)),
            flag_after(
                &mut world,
                player,
                &ServerboundMovePlayerPos {
                    position: Default::default(),
                    flags: flags(false),
                },
            ),
            flag_after(
                &mut world,
                player,
                &ServerboundMovePlayerPosRot {
                    position: Default::default(),
                    look: Default::default(),
                    flags: flags(true),
                },
            ),
            flag_after(
                &mut world,
                player,
                &ServerboundMovePlayerRot {
                    look: Default::default(),
                    flags: flags(false),
                },
            ),
            flag_after(&mut world, player, &vehicle(true)),
        ];

        assert_eq!(
            reported,
            [true, true, false, true, false, false],
            "BRK-PRG-002"
        );
    }
}
