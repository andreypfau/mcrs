use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundDamageEvent, ClientboundEntityEvent,
    ClientboundEntityPositionSync, ClientboundHurtAnimation, ClientboundLogin,
    ClientboundMoveEntityPos, ClientboundMoveEntityPosRot, ClientboundMoveEntityRot,
    ClientboundRemoveEntities, ClientboundRespawn, ClientboundRotateHead, ClientboundSetEntityData,
    ClientboundSetEquipment, ClientboundTeleportEntity, ClientboundTickingState,
    ClientboundTickingStep, ClientboundUpdateAttributes, VecDelta,
};
use mcrs_minecraft_protocol_game::{ByteAngle, PositionFlag};

use crate::client_entity::{ClientEntity, NetworkEntities};
use crate::delta;
use crate::equipment::Equipment;
use crate::living::Living;
use crate::motion::{Age, Motion};
use crate::path::PacketPath;
use crate::placement::{Placement, Rotation};
use crate::server_ticking::ServerTicking;
use crate::synced_data::SyncedData;
use crate::tick::in_ticking_area;

/// A position farther than this from the current one is a jump, not a movement.
pub const SNAP_DISTANCE: f64 = 64.0;
/// The largest X and Z of an added entity.
pub const WORLD_LIMIT: f64 = 3.0e7;
/// The entity event that sets the health of a living entity to 0.
pub const DEATH_EVENT: i8 = 3;

pub(crate) fn add_observers(app: &mut bevy_app::App) {
    app.add_observer(on_add_entity)
        .add_observer(on_remove_entities)
        .add_observer(on_move_entity_pos)
        .add_observer(on_move_entity_pos_rot)
        .add_observer(on_move_entity_rot)
        .add_observer(on_position_sync)
        .add_observer(on_teleport_entity)
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
    ));
    match client.type_data() {
        Some(data) if data.is_living() => {
            spawned.insert((Living::new(degrees(packet.head_yaw)), Equipment::default()));
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

/// Starts an interpolation of the entity `network_id` from a relative move.
fn relative_move(
    network_id: i32,
    motion_delta: Option<&VecDelta>,
    rotation: Option<Rotation>,
    on_ground: bool,
    entities: &NetworkEntities,
    query: &mut Query<(&ClientEntity, &mut Motion)>,
) {
    let Some(entity) = entities.get(network_id) else {
        return;
    };
    let Ok((client, mut motion)) = query.get_mut(entity) else {
        return;
    };
    let path = motion_delta.map(|motion_delta| delta::motion_path(motion.delta_base, motion_delta));
    if let Some(end) = path.as_ref().and_then(PacketPath::end) {
        motion.delta_base = end;
    }
    motion.start(path, rotation, client.update_interval());
    motion.on_ground = on_ground;
}

fn on_move_entity_pos(
    event: On<Received<ClientboundMoveEntityPos>>,
    entities: Res<NetworkEntities>,
    mut query: Query<(&ClientEntity, &mut Motion)>,
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
    mut query: Query<(&ClientEntity, &mut Motion)>,
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
    mut query: Query<(&ClientEntity, &mut Motion)>,
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

fn on_position_sync(
    event: On<Received<ClientboundEntityPositionSync>>,
    entities: Res<NetworkEntities>,
    columns: Option<Res<ColumnStore>>,
    mut query: Query<(&ClientEntity, &mut Motion)>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Some(entity) = entities.get(packet.entity_id.0) else {
        return;
    };
    let Ok((client, mut motion)) = query.get_mut(entity) else {
        return;
    };
    let path = delta::sync_path(&packet.position);
    let Some(end) = path.end() else {
        return;
    };
    motion.delta_base = end;
    let rotation = Rotation {
        yaw: packet.look.yaw,
        pitch: packet.look.pitch,
    };
    // A frozen entity stays in the ticking area: a capture shows a frozen pig that keeps its path.
    let ticking_area = in_ticking_area(columns.as_deref(), motion.current.position);
    if !ticking_area || motion.current.position.distance(end) > SNAP_DISTANCE {
        motion.snap(Placement::new(end, rotation.yaw, rotation.pitch));
    } else {
        motion.start(Some(path), Some(rotation), client.update_interval());
    }
    motion.on_ground = packet.on_ground;
}

fn on_teleport_entity(
    event: On<Received<ClientboundTeleportEntity>>,
    entities: Res<NetworkEntities>,
    mut query: Query<(&ClientEntity, &mut Motion)>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Some(entity) = entities.get(packet.entity_id.0) else {
        return;
    };
    let Ok((client, mut motion)) = query.get_mut(entity) else {
        return;
    };
    let current = motion.current;
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
    let rotation = Rotation {
        yaw: relative(
            PositionFlag::YRot,
            f64::from(packet.look.yaw),
            f64::from(current.yaw),
        ) as f32,
        pitch: relative(
            PositionFlag::XRot,
            f64::from(packet.look.pitch),
            f64::from(current.pitch),
        ) as f32,
    };
    if current.position.distance(position) > SNAP_DISTANCE {
        motion.snap(Placement::new(position, rotation.yaw, rotation.pitch));
    } else {
        motion.start(
            Some(PacketPath::Linear(position)),
            Some(rotation),
            client.update_interval(),
        );
    }
    motion.on_ground = packet.on_ground;
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
        data.apply(client.type_data(), &packet.metadata.0);
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
    mut data: Query<&mut SyncedData>,
) {
    if let Some(packet) = event.packet()
        && let Some(entity) = entities.get(packet.entity_id.0)
        && let Ok(mut data) = data.get_mut(entity)
    {
        data.apply_attributes(&packet.attributes);
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
