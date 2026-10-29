use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::prelude::{
    Changed, DetectChangesMut, Entity, Message, MessageReader, MessageWriter, Mut, On, Query, With,
};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::tick::TickSet;
use mcrs_minecraft_level::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::Look;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot, ServerboundMovePlayerStatusOnly,
};

use crate::world::entity::player::dimension_change::CarryOnDimensionChangeExt;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine::session::HostAnchor;
use mcrs_minecraft_engine::teleport::{MAX_XZ, MAX_Y, TeleportState};

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_move_pos);
        app.add_observer(handle_move_pos_rot);
        app.add_observer(handle_move_rot);
        app.add_observer(handle_move_status_only);
        app.add_observer(handle_accept_teleportation);
        app.add_message::<PlayerMovement>();
        app.carry_on_dimension_change::<TeleportState>();
        app.add_systems(FixedUpdate, process_movement.in_set(TickSet::Input));
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
    mut query: Query<(Mut<TeleportState>, Mut<Transform>)>,
) {
    const MAX_POS: DVec3 = DVec3::new(MAX_XZ, MAX_Y, MAX_XZ);
    const MIN_POS: DVec3 = DVec3::new(-MAX_XZ, -MAX_Y, -MAX_XZ);

    reader.read().for_each(|m| {
        let Ok((mut state, mut transform)) = query.get_mut(m.entity) else {
            return;
        };
        if state.pending_teleports() > 0 {
            return;
        }
        if let Some(p) = m.position {
            transform.set_if_neq(transform.with_translation(p.clamp(MIN_POS, MAX_POS)));
        }
        if let Some(l) = m.look {
            transform.set_if_neq(transform.with_rotation(l));
        }
        state.sync(*transform);
    })
}

fn teleport(
    mut clients: Query<
        (&HostAnchor, &mut TeleportState, &Transform),
        (Changed<Transform>, With<HostAnchor>),
    >,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (anchor, mut state, transform) in &mut clients {
        let synced = state.synced_transform();
        let changed_pos = transform.translation != synced.translation;
        let changed_rot = transform.rotation != synced.rotation;

        if changed_pos || changed_rot {
            let teleport_id = state.sent(*transform);
            packet_writer.write(
                to(
                    anchor.0,
                    PacketPayload::PlayerPosition(ClientboundPlayerPosition {
                        teleport_id: VarInt(teleport_id),
                        position: transform.translation,
                        velocity: DVec3::ZERO,
                        look: Look::default(),
                        flags: mcrs_minecraft_protocol::PositionFlags::default(),
                    }),
                )
                .critical(),
            );
        }
    }
}

/// Decrement the pending-teleport counter when the client confirms a teleport.
/// The server increments `pending_teleports` for every server-authoritative
/// position it sends (login sync and each `teleport` emit); without this the
/// counter would grow without bound and any gate on it would block the player
/// permanently. `event.entity` is the in-dim player entity, matching the
/// `TeleportState` owner.
fn handle_accept_teleportation(
    event: On<Received<ServerboundAcceptTeleportation>>,
    mut players: Query<&mut TeleportState>,
) {
    if event.packet().is_none() {
        return;
    }
    if let Ok(mut state) = players.get_mut(event.entity) {
        state.confirm();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_minecraft_engine::bus::PacketTarget;

    #[test]
    fn a_server_move_sends_one_position_with_the_next_teleport_id_and_a_synced_one_none() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerPacket>>();
        let moved_anchor = world.spawn_empty().id();
        let moved = world
            .spawn((
                HostAnchor(moved_anchor),
                Transform::default().with_translation(DVec3::new(1.0, 0.0, 0.0)),
                TeleportState::default(),
            ))
            .id();
        let synced_anchor = world.spawn_empty().id();
        let synced = world
            .spawn((
                HostAnchor(synced_anchor),
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
            matches!(sent[0].target, PacketTarget::SinglePlayer(anchor) if anchor == moved_anchor)
        );
        let PacketPayload::PlayerPosition(position) = &sent[0].data else {
            panic!("a move is sent as a player position");
        };
        assert_eq!(position.teleport_id.0, 0);
        assert_eq!(position.position, DVec3::new(1.0, 0.0, 0.0));

        let state = world.get::<TeleportState>(moved).unwrap();
        assert_eq!(
            (state.pending_teleports(), state.teleport_id_counter()),
            (1, 1)
        );
        let state = world.get::<TeleportState>(synced).unwrap();
        assert_eq!(
            (state.pending_teleports(), state.teleport_id_counter()),
            (0, 0)
        );
    }

    #[test]
    fn a_client_move_applies_only_once_every_teleport_is_confirmed() {
        let mut world = World::new();
        world.init_resource::<Messages<PlayerMovement>>();
        let awaiting = world
            .spawn((Transform::default(), TeleportState::after_login()))
            .id();
        let confirmed = world
            .spawn((Transform::default(), TeleportState::default()))
            .id();
        let to = DVec3::new(5.0, 64.0, 5.0);
        for entity in [awaiting, confirmed] {
            world.write_message(PlayerMovement {
                entity,
                position: Some(to),
                look: None,
            });
        }

        world.run_system_once(process_movement).unwrap();

        let at = |entity| world.get::<Transform>(entity).unwrap().translation;
        assert_eq!((at(awaiting), at(confirmed)), (DVec3::ZERO, to));
    }
}
