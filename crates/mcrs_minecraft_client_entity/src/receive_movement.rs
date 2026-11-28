use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::{
    PacketPath, Placement, Rotation, SNAP_DISTANCE, in_ticking_area,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_physics::{Orientation, PlayerState, Relative, Teleport, apply_teleport};
use mcrs_minecraft_protocol_game::PositionFlag;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    ClientboundMoveEntityRot, ClientboundSetEntityMotion, ClientboundTeleportEntity, VecDelta,
};

use crate::delta;
use crate::moved_entity::MovedEntity;
use crate::network_entities::NetworkEntities;
use crate::receive::degrees;

pub(crate) fn add_observers(app: &mut bevy_app::App) {
    app.add_observer(on_move_entity_pos)
        .add_observer(on_move_entity_pos_rot)
        .add_observer(on_move_entity_rot)
        .add_observer(on_position_sync)
        .add_observer(on_teleport_entity)
        .add_observer(on_set_entity_motion);
}

fn relative_move(
    network_id: i32,
    motion_delta: Option<&VecDelta>,
    rotation: Option<Rotation>,
    on_ground: bool,
    entities: &NetworkEntities,
    mut query: Query<'_, '_, MovedEntity>,
) {
    let Some(entity) = entities.get(network_id) else {
        return;
    };
    let Ok(mut moved) = query.get_mut(entity) else {
        return;
    };
    let path =
        motion_delta.map(|motion_delta| delta::motion_path(moved.motion.delta_base, motion_delta));
    if let Some(end) = path.as_ref().and_then(PacketPath::end)
        && moved.motion.delta_base != end
    {
        moved.motion.delta_base = end;
    }
    moved.update(path, rotation);
    moved.set_on_ground(on_ground);
}

fn on_move_entity_pos(
    event: On<Received<ClientboundMoveEntityPos>>,
    entities: Res<NetworkEntities>,
    mut query: Query<MovedEntity>,
) {
    if let Some(packet) = event.packet() {
        relative_move(
            packet.entity_id.0,
            Some(&packet.delta),
            None,
            packet.on_ground,
            &entities,
            query.reborrow(),
        );
    }
}

fn on_move_entity_pos_rot(
    event: On<Received<ClientboundMoveEntityPosRot>>,
    entities: Res<NetworkEntities>,
    mut query: Query<MovedEntity>,
) {
    if let Some(packet) = event.packet() {
        let rotation = Rotation {
            yaw: degrees(packet.y_rot),
            pitch: degrees(packet.x_rot),
        };
        relative_move(
            packet.entity_id.0,
            Some(&packet.delta),
            Some(rotation),
            packet.on_ground,
            &entities,
            query.reborrow(),
        );
    }
}

fn on_move_entity_rot(
    event: On<Received<ClientboundMoveEntityRot>>,
    entities: Res<NetworkEntities>,
    mut query: Query<MovedEntity>,
) {
    if let Some(packet) = event.packet() {
        let rotation = Rotation {
            yaw: degrees(packet.y_rot),
            pitch: degrees(packet.x_rot),
        };
        relative_move(
            packet.entity_id.0,
            None,
            Some(rotation),
            packet.on_ground,
            &entities,
            query.reborrow(),
        );
    }
}

fn snaps(from: DVec3, to: DVec3) -> bool {
    from.distance_squared(to) > SNAP_DISTANCE * SNAP_DISTANCE
}

fn on_position_sync(
    event: On<Received<ClientboundEntityPositionSync>>,
    entities: Res<NetworkEntities>,
    columns: Option<Res<ColumnStore>>,
    mut query: Query<MovedEntity>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Some(entity) = entities.get(packet.entity_id.0) else {
        return;
    };
    let Ok(mut moved) = query.get_mut(entity) else {
        return;
    };
    let path = delta::sync_path(&packet.position);
    let Some(end) = path.end() else {
        return;
    };
    if moved.motion.delta_base != end {
        moved.motion.delta_base = end;
    }
    let rotation = Rotation {
        yaw: packet.look.yaw,
        pitch: packet.look.pitch,
    };
    let current = moved.current().position;
    if !in_ticking_area(columns.as_deref(), current) || snaps(current, end) {
        moved.snap(Placement::new(end, rotation.yaw, rotation.pitch), end);
    } else {
        moved.update(Some(path), Some(rotation));
    }
    moved.set_on_ground(packet.on_ground);
}

fn on_teleport_entity(
    event: On<Received<ClientboundTeleportEntity>>,
    entities: Res<NetworkEntities>,
    mut query: Query<MovedEntity>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Some(entity) = entities.get(packet.entity_id.0) else {
        return;
    };
    let Ok(mut moved) = query.get_mut(entity) else {
        return;
    };
    let current = moved.current();
    let reference = moved.reference();
    let mut state = PlayerState::default();
    state.body.position = reference.position;
    state.body.velocity = moved.velocity();
    let mut orientation = Orientation::facing(reference.yaw, reference.pitch);
    let teleport = Teleport {
        position: packet.position,
        velocity: packet.velocity,
        yaw: packet.look.yaw,
        pitch: packet.look.pitch,
        relative: Relative::from_bits_truncate(packet.relatives.bits()),
    };
    apply_teleport(&mut state, &mut orientation, &teleport);
    let position = state.body.position;
    let velocity = state.body.velocity;
    let placement = Placement::new(position, orientation.yaw, orientation.pitch);
    if snaps(current.position, position) {
        let previous = moved.motion.previous.position;
        let moved_by = position - current.position;
        let previous_axis = |flag: PositionFlag, previous: f64, moved_by: f64, new: f64| {
            if packet.relatives.contains(flag) {
                previous + moved_by
            } else {
                new
            }
        };
        let previous_position = DVec3::new(
            previous_axis(PositionFlag::X, previous.x, moved_by.x, position.x),
            previous_axis(PositionFlag::Y, previous.y, moved_by.y, position.y),
            previous_axis(PositionFlag::Z, previous.z, moved_by.z, position.z),
        );
        moved.snap(placement, previous_position);
    } else {
        moved.update(
            Some(PacketPath::Linear(position)),
            Some(placement.rotation()),
        );
    }
    moved.set_velocity(velocity);
    moved.set_on_ground(packet.on_ground);
}

fn on_set_entity_motion(
    event: On<Received<ClientboundSetEntityMotion>>,
    entities: Res<NetworkEntities>,
    connections: Query<&JoinedGame>,
    mut query: Query<MovedEntity>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let network_id = packet.entity_id.0;
    let Some(entity) = entities.get(network_id) else {
        return;
    };
    let own = connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == network_id);
    if own {
        return;
    }
    let Ok(mut moved) = query.get_mut(entity) else {
        return;
    };
    let velocity = packet.velocity.0;
    if moved.client.kind == EntityType::Player {
        moved.approach_velocity(velocity);
    } else {
        moved.set_velocity(velocity);
    }
}
