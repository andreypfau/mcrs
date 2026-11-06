use crate::bus::to;
use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::prelude::{
    Changed, DetectChangesMut, Entity, Message, MessageReader, MessageWriter, Mut, On, Query, Res,
    With,
};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_math::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot, ServerboundMovePlayerStatusOnly,
};

use crate::bus::{OutboundPlayerPacket, PacketPayload};
use crate::coordinator::clock::GameTime;
use crate::teleport::{AcceptOutcome, MAX_XZ, MAX_Y, TeleportState};
use mcrs_minecraft_engine_world::session::Owner;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_move_pos);
        app.add_observer(handle_move_pos_rot);
        app.add_observer(handle_move_rot);
        app.add_observer(handle_move_status_only);
        app.add_observer(handle_accept_teleportation);
        app.add_message::<PlayerMovement>();
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
        });
    }
}

fn handle_move_status_only(
    on: On<Received<ServerboundMovePlayerStatusOnly>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if on.packet().is_some() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: None,
            look: None,
        });
    }
}

#[derive(Message)]
pub struct PlayerMovement {
    entity: Entity,
    position: Option<DVec3>,
    look: Option<Rotation>,
}

pub(crate) fn process_movement(
    mut reader: MessageReader<PlayerMovement>,
    mut query: Query<(&Owner, Mut<TeleportState>, Mut<Transform>)>,
    time: Res<GameTime>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    const MAX_POS: DVec3 = DVec3::new(MAX_XZ, MAX_Y, MAX_XZ);
    const MIN_POS: DVec3 = DVec3::new(-MAX_XZ, -MAX_Y, -MAX_XZ);

    for movement in reader.read() {
        let Ok((owner, mut state, mut transform)) = query.get_mut(movement.entity) else {
            continue;
        };
        if state.pending().is_some() {
            if let Some(destination) = state.resend_due(*time) {
                let id = state.sent(destination, *time);
                packets.write(position_packet(owner, id, destination));
            }
            continue;
        }
        if let Some(position) = movement.position {
            transform.set_if_neq(transform.with_translation(position.clamp(MIN_POS, MAX_POS)));
        }
        if let Some(look) = movement.look {
            transform.set_if_neq(transform.with_rotation(look));
        }
        state.sync(*transform);
    }
}

fn position_packet(
    owner: &Owner,
    teleport_id: i32,
    destination: Transform,
) -> OutboundPlayerPacket {
    to(
        owner.0,
        PacketPayload::PlayerPosition(ClientboundPlayerPosition {
            teleport_id: VarInt(teleport_id),
            position: destination.translation,
            velocity: DVec3::ZERO,
            look: Look::default(),
            flags: mcrs_minecraft_protocol_game::PositionFlags::default(),
        }),
    )
    .critical()
}

fn teleport(
    mut clients: Query<(&Owner, &mut TeleportState, &Transform), (Changed<Transform>, With<Owner>)>,
    time: Res<GameTime>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (owner, mut state, transform) in &mut clients {
        let synced = state.synced_transform();
        let changed_pos = transform.translation != synced.translation;
        let changed_rot = transform.rotation != synced.rotation;

        if changed_pos || changed_rot {
            let teleport_id = state.sent(*transform, *time);
            packet_writer.write(position_packet(owner, teleport_id, *transform));
        }
    }
}

/// An accept of the pending teleport at its destination confirms it, so moves apply again. An
/// accept at another position gets a correction: the position of an accept never becomes the
/// position of the player.
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
    if let AcceptOutcome::Mismatch { destination } =
        state.accept(accept.teleport_id.0, accept.position.into())
    {
        let id = state.sent(destination, *time);
        packets.write(position_packet(owner, id, destination));
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
            .spawn((Owner(PlayerSession(1)), Transform::default(), unconfirmed))
            .id();
        let confirmed = world
            .spawn((
                Owner(PlayerSession(2)),
                Transform::default(),
                TeleportState::default(),
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
    fn a_move_during_a_stale_teleport_resends_it_with_the_next_id() {
        let mut world = movement_world();
        let destination = Transform::default().with_translation(DVec3::new(3.0, 64.0, 0.0));
        let mut state = TeleportState::default();
        state.sent(destination, GameTime(0));
        let player = world
            .spawn((Owner(PlayerSession(1)), destination, state))
            .id();
        world.insert_resource(GameTime(20));
        world.write_message(movement(player, DVec3::new(9.0, 64.0, 0.0)));

        world.run_system_once(process_movement).unwrap();

        assert_eq!(
            sent_ids(&mut world),
            [2],
            "TP-CONFIRM-RES-002, TP-CONFIRM-ID-002"
        );
        let state = world.get::<TeleportState>(player).unwrap();
        assert_eq!(
            state.pending().map(|pending| pending.destination),
            Some(destination),
            "TP-CONFIRM-RES-004"
        );
        assert_eq!(world.get::<Transform>(player), Some(&destination));
    }
}
