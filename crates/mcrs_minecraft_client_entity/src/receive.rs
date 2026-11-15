use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::attribute_instances::AttributeInstances;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::{
    Age, ClientEntity, Equipment, Living, Motion, PacketPath, Placement, Rotation, SNAP_DISTANCE,
    ServerTicking, SyncedData, in_ticking_area,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundDamageEvent, ClientboundEntityEvent,
    ClientboundEntityPositionSync, ClientboundHurtAnimation, ClientboundLogin,
    ClientboundMoveEntityPos, ClientboundMoveEntityPosRot, ClientboundMoveEntityRot,
    ClientboundRemoveEntities, ClientboundRespawn, ClientboundRotateHead, ClientboundSetEntityData,
    ClientboundSetEntityMotion, ClientboundSetEquipment, ClientboundTeleportEntity,
    ClientboundTickingState, ClientboundTickingStep, ClientboundUpdateAttributes, VecDelta,
};
use mcrs_minecraft_protocol_game::{ByteAngle, PositionFlag};

use crate::add_entity_data::AddEntityData;
use crate::apply_entity_data::apply_entity_data;
use crate::delta;
use crate::moved_entity::MovedEntity;
use crate::network_entities::NetworkEntities;
use crate::orb_interpolation::OrbInterpolation;
use crate::simulated_motion::simulated_motion;
use crate::velocity::Velocity;

/// The largest X and Z of an added entity.
pub const WORLD_LIMIT: f64 = 3.0e7;
/// The entity event that sets the health of a living entity to 0.
pub const DEATH_EVENT: i8 = ClientboundEntityEvent::DIED;

pub(crate) fn add_observers(app: &mut bevy_app::App) {
    app.add_observer(on_add_entity)
        .add_observer(on_remove_entities)
        .add_observer(on_move_entity_pos)
        .add_observer(on_move_entity_pos_rot)
        .add_observer(on_move_entity_rot)
        .add_observer(on_position_sync)
        .add_observer(on_teleport_entity)
        .add_observer(on_set_entity_motion)
        .add_observer(on_rotate_head)
        .add_observer(on_set_entity_data)
        .add_observer(on_set_equipment)
        .add_observer(on_update_attributes)
        .add_observer(on_damage_event)
        .add_observer(on_hurt_animation)
        .add_observer(on_entity_event)
        .add_observer(on_ticking_state)
        .add_observer(on_ticking_step)
        .add_observer(on_login)
        .add_observer(on_respawn);
}

fn degrees(angle: ByteAngle) -> f32 {
    angle.to_degrees()
}

fn on_add_entity(
    event: On<Received<ClientboundAddEntity>>,
    mut entities: ResMut<NetworkEntities>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let client = ClientEntity {
        network_id: packet.id.0,
        uuid: packet.uuid,
        kind: packet.kind,
    };
    let position = DVec3::new(
        packet.pos.x.clamp(-WORLD_LIMIT, WORLD_LIMIT),
        packet.pos.y,
        packet.pos.z.clamp(-WORLD_LIMIT, WORLD_LIMIT),
    );
    let placement = Placement::new(
        position,
        degrees(packet.yaw),
        degrees(packet.pitch).clamp(-90.0, 90.0),
    );
    let mut spawned = commands.spawn((
        client,
        Motion::new(placement, packet.pos),
        Age::default(),
        SyncedData::default(),
        AddEntityData(packet.data.0),
    ));
    let velocity = packet.movement.0;
    match simulated_motion(&client, position, velocity) {
        Some(motion) => {
            spawned.insert(motion);
            if client.kind == EntityType::ExperienceOrb {
                spawned.insert(OrbInterpolation::default());
            }
        }
        None => {
            spawned.insert(Velocity::new(velocity));
        }
    }
    match client.type_data() {
        Some(data) if data.is_living() => {
            spawned.insert((Living::new(degrees(packet.head_yaw)), Equipment::default()));
            if let Some(bases) = data.attribute_bases() {
                spawned.insert(AttributeInstances::new(bases));
            }
        }
        Some(_) => {}
        None => tracing::debug!(
            kind = client.kind.as_static_str(),
            "the entity type has no type data; the entity does not tick as a living entity"
        ),
    }
    let spawned = spawned.id();
    if let Some(replaced) = entities.insert(client.network_id, spawned) {
        commands.entity(replaced).despawn();
    }
}

fn on_remove_entities(
    event: On<Received<ClientboundRemoveEntities>>,
    mut entities: ResMut<NetworkEntities>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    for id in packet.entity_ids {
        if let Some(entity) = entities.remove(id.0) {
            commands.entity(entity).despawn();
        }
    }
}

fn clear_entities(entities: &mut NetworkEntities, commands: &mut Commands) {
    for entity in entities.drain() {
        commands.entity(entity).despawn();
    }
}

fn on_login(
    _event: On<Received<ClientboundLogin>>,
    mut entities: ResMut<NetworkEntities>,
    mut commands: Commands,
) {
    clear_entities(&mut entities, &mut commands);
}

fn on_respawn(
    _event: On<Received<ClientboundRespawn>>,
    mut entities: ResMut<NetworkEntities>,
    mut commands: Commands,
) {
    clear_entities(&mut entities, &mut commands);
}

/// Applies a relative move to the entity `network_id`.
fn relative_move(
    network_id: i32,
    motion_delta: Option<&VecDelta>,
    rotation: Option<Rotation>,
    on_ground: bool,
    entities: &NetworkEntities,
    query: &mut Query<MovedEntity>,
) {
    let Some(entity) = entities.get(network_id) else {
        return;
    };
    let Ok(mut moved) = query.get_mut(entity) else {
        return;
    };
    let path =
        motion_delta.map(|motion_delta| delta::motion_path(moved.motion.delta_base, motion_delta));
    if let Some(end) = path.as_ref().and_then(PacketPath::end) {
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
            &mut query,
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
            &mut query,
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
            &mut query,
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
    moved.motion.delta_base = end;
    let rotation = Rotation {
        yaw: packet.look.yaw,
        pitch: packet.look.pitch,
    };
    let current = moved.current().position;
    // A frozen entity stays in the ticking area: a capture shows a frozen pig that keeps its path.
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
    let current_velocity = moved.velocity();
    let relative = |flag: PositionFlag, value: f64, base: f64| {
        if packet.relatives.contains(flag) {
            base + value
        } else {
            value
        }
    };
    let position = DVec3::new(
        relative(PositionFlag::X, packet.position.x, current.position.x),
        relative(PositionFlag::Y, packet.position.y, current.position.y),
        relative(PositionFlag::Z, packet.position.z, current.position.z),
    );
    let yaw = relative(
        PositionFlag::YRot,
        f64::from(packet.look.yaw),
        f64::from(current.yaw),
    ) as f32;
    let pitch = relative(
        PositionFlag::XRot,
        f64::from(packet.look.pitch),
        f64::from(current.pitch),
    ) as f32;
    let velocity = DVec3::new(
        relative(PositionFlag::DeltaX, packet.velocity.x, current_velocity.x),
        relative(PositionFlag::DeltaY, packet.velocity.y, current_velocity.y),
        relative(PositionFlag::DeltaZ, packet.velocity.z, current_velocity.z),
    );
    let placement = Placement::new(position, yaw, pitch);
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

fn on_rotate_head(
    event: On<Received<ClientboundRotateHead>>,
    entities: Res<NetworkEntities>,
    mut living: Query<&mut Living>,
) {
    if let Some(packet) = event.packet()
        && let Some(entity) = entities.get(packet.entity_id.0)
        && let Ok(mut living) = living.get_mut(entity)
    {
        living.head_turn.start(degrees(packet.y_head_rot));
    }
}

fn on_set_entity_data(
    event: On<Received<ClientboundSetEntityData<'static>>>,
    entities: Res<NetworkEntities>,
    mut query: Query<(&ClientEntity, &mut SyncedData)>,
) {
    if let Some(packet) = event.packet()
        && let Some(entity) = entities.get(packet.entity_id.0)
        && let Ok((client, mut data)) = query.get_mut(entity)
    {
        apply_entity_data(&mut data, client.type_data(), &packet.metadata.0);
    }
}

fn on_set_equipment(
    event: On<Received<ClientboundSetEquipment>>,
    entities: Res<NetworkEntities>,
    mut equipment: Query<&mut Equipment>,
) {
    if let Some(packet) = event.packet()
        && let Some(entity) = entities.get(packet.entity_id.0)
        && let Ok(mut equipment) = equipment.get_mut(entity)
    {
        for (slot, stack) in packet.slots {
            equipment.set(slot, stack);
        }
    }
}

fn on_update_attributes(
    event: On<Received<ClientboundUpdateAttributes<'static>>>,
    entities: Res<NetworkEntities>,
    connections: Query<&JoinedGame>,
    mut living: Query<(&mut AttributeInstances, Option<&mut SyncedData>)>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let network_id = packet.entity_id.0;
    let own = connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == network_id);
    if own {
        return;
    }
    let Some(entity) = entities.get(network_id) else {
        return;
    };
    let Ok((mut instances, data)) = living.get_mut(entity) else {
        return;
    };
    for entry in &packet.attributes {
        instances.receive(entry);
    }
    if let Some(data) = data
        && let Some(scale) = instances.value(Attribute::Scale)
    {
        data.map_unchanged(|data| &mut data.scale).set_if_neq(scale);
    }
}

fn on_damage_event(
    event: On<Received<ClientboundDamageEvent>>,
    entities: Res<NetworkEntities>,
    mut living: Query<&mut Living>,
) {
    if let Some(packet) = event.packet()
        && let Some(entity) = entities.get(packet.entity_id.0)
        && let Ok(mut living) = living.get_mut(entity)
    {
        living.hurt();
        living.walk.hurt();
    }
}

fn on_hurt_animation(
    event: On<Received<ClientboundHurtAnimation>>,
    entities: Res<NetworkEntities>,
    mut living: Query<&mut Living>,
) {
    if let Some(packet) = event.packet()
        && let Some(entity) = entities.get(packet.entity_id.0)
        && let Ok(mut living) = living.get_mut(entity)
    {
        living.hurt();
    }
}

fn on_entity_event(
    event: On<Received<ClientboundEntityEvent>>,
    entities: Res<NetworkEntities>,
    mut data: Query<&mut SyncedData, With<Living>>,
) {
    if let Some(packet) = event.packet()
        && packet.entity_status == DEATH_EVENT
        && let Some(entity) = entities.get(packet.entity_id)
        && let Ok(mut data) = data.get_mut(entity)
    {
        data.health = 0.0;
    }
}

fn on_ticking_state(
    event: On<Received<ClientboundTickingState>>,
    mut ticking: ResMut<ServerTicking>,
) {
    if let Some(packet) = event.packet() {
        ticking.tick_rate = packet.tick_rate;
        ticking.frozen = packet.frozen;
    }
}

fn on_ticking_step(
    event: On<Received<ClientboundTickingStep>>,
    mut ticking: ResMut<ServerTicking>,
) {
    if let Some(packet) = event.packet() {
        ticking.steps = u32::try_from(packet.steps.0).unwrap_or(0);
    }
}
