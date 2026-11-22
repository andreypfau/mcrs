use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::apply_entity_data::apply_entity_data;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::session::InSession;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{AttributeInstances, InstanceChange};
use mcrs_minecraft_entity_key::{Cause, EntityUuid};
use mcrs_minecraft_entity_visual::{
    Age, ClientEntity, Equipment, Living, Motion, Placement, PlayerPoseTicks, ServerTicking,
    SleepingBed, SyncedData, TickVelocity,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundDamageEvent, ClientboundEntityEvent, ClientboundHurtAnimation,
    ClientboundLogin, ClientboundRemoveEntities, ClientboundRespawn, ClientboundRotateHead,
    ClientboundSetEntityData, ClientboundSetEquipment, ClientboundTickingState,
    ClientboundTickingStep, ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::entity_data_index;

use crate::add_entity_data::AddEntityData;
use crate::entity_damaged::EntityDamaged;
use crate::fuse::{FUSE_INDEX, Fuse};
use crate::item_entity_stack::{ITEM_STACK_INDEX, ItemEntityStack};
use crate::network_entities::NetworkEntities;
use crate::orb_interpolation::OrbInterpolation;
use crate::receive_movement;
use crate::simulated_motion::simulated_motion;
use crate::tracked_velocity::TrackedVelocity;

pub const WORLD_LIMIT: f64 = 3.0e7;
pub const DEATH_EVENT: i8 = ClientboundEntityEvent::DIED;

pub(crate) fn add_observers(app: &mut bevy_app::App) {
    app.add_observer(on_add_entity)
        .add_observer(on_remove_entities)
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
    receive_movement::add_observers(app);
}

pub(crate) fn degrees(angle: ByteAngle) -> f32 {
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
        InSession(event.entity),
    ));
    let velocity = packet.movement.0;
    match simulated_motion(&client, position, velocity) {
        Some(motion) => {
            spawned.insert(motion);
            match client.kind {
                EntityType::ExperienceOrb => {
                    spawned.insert(OrbInterpolation::default());
                }
                EntityType::Tnt => {
                    spawned.insert(Fuse::default());
                }
                EntityType::Item => {
                    spawned.insert(ItemEntityStack::default());
                }
                EntityType::FallingBlock => {
                    spawned.insert(AddEntityData(packet.data.0));
                }
                _ => {}
            }
        }
        None => {
            spawned.insert(TrackedVelocity::new(velocity));
        }
    }
    if client.kind == EntityType::Player {
        spawned.insert((
            PlayerPoseTicks::default(),
            TickVelocity::new(velocity),
            SleepingBed::default(),
        ));
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
    mut query: Query<(
        &ClientEntity,
        &mut SyncedData,
        Option<&mut Fuse>,
        Option<&mut ItemEntityStack>,
        Option<&mut MotionState>,
    )>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Some(entity) = entities.get(packet.entity_id.0) else {
        return;
    };
    let Ok((client, mut data, mut fuse, mut stack, mut motion)) = query.get_mut(entity) else {
        return;
    };
    apply_entity_data(&mut data, client.type_data(), &packet.metadata.0);
    for entry in &packet.metadata.0 {
        match (
            entry.index,
            &entry.value,
            fuse.as_mut(),
            stack.as_mut(),
            motion.as_mut(),
        ) {
            (FUSE_INDEX, MetaDataValue::VarInt(ticks), Some(fuse), _, _) => fuse.0 = ticks.0,
            (ITEM_STACK_INDEX, MetaDataValue::Slot(item), _, Some(stack), _) => {
                stack.0 = item.clone();
            }
            (
                entity_data_index::NO_GRAVITY,
                MetaDataValue::Boolean(no_gravity),
                _,
                _,
                Some(motion),
            ) => {
                motion
                    .reborrow()
                    .map_unchanged(|MotionState(state)| &mut state.no_gravity)
                    .set_if_neq(*no_gravity);
            }
            _ => {}
        }
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
    let mut change = InstanceChange::Unchanged;
    for entry in &packet.attributes {
        match instances.bypass_change_detection().receive(entry) {
            Ok(InstanceChange::Changed) => change = InstanceChange::Changed,
            Ok(InstanceChange::Unchanged) => {}
            Err(error) => {
                tracing::warn!(%error, "the server sent an attribute entry that the client skips");
            }
        }
    }
    if change == InstanceChange::Changed {
        instances.set_changed();
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
    connections: Query<&JoinedGame>,
    own_player: Query<Entity, With<Player>>,
    clients: Query<&ClientEntity>,
    mut living: Query<&mut Living>,
    mut damaged: MessageWriter<EntityDamaged>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let network_id = packet.entity_id.0;
    let own = connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == network_id);
    let entity = if own {
        own_player.single().ok()
    } else {
        entities.get(network_id)
    };
    let Some(entity) = entity else {
        return;
    };
    if let Ok(mut living) = living.get_mut(entity) {
        living.hurt();
        living.walk.hurt();
    }
    let Ok(damage_type) = u32::try_from(packet.damage_type.0) else {
        return;
    };
    let uuid_of = |shifted_id: i32| {
        let network_id = shifted_id.checked_sub(1)?;
        let entity = entities.get(network_id)?;
        let client = clients.get(entity).ok()?;
        Some(EntityUuid(client.uuid))
    };
    damaged.write(EntityDamaged {
        entity,
        damage_type,
        cause: Cause {
            causing: uuid_of(packet.cause_entity_id.0),
            direct: uuid_of(packet.direct_entity_id.0),
        },
        source_position: packet.source_position,
    });
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
