use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use uuid::Uuid;

use crate::client_entity::ClientEntity;
use crate::equipment::Equipment;
use crate::living::Living;
use crate::motion::{Age, Motion};
use crate::placement::Placement;
use crate::synced_data::SyncedData;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostEntity;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CameraEntity;

const HOST_NETWORK_ID: i32 = i32::MIN;

pub const SNAP_DISTANCE: f64 = 64.0;

#[must_use]
pub fn host_living_entity(
    kind: EntityType,
    uuid: Uuid,
    placement: Placement,
) -> Option<impl Bundle> {
    let client = ClientEntity {
        network_id: HOST_NETWORK_ID,
        uuid,
        kind,
    };
    client.type_data().filter(|data| data.is_living())?;
    Some((
        HostEntity,
        client,
        Motion::new(placement, placement.position),
        Age::default(),
        SyncedData::default(),
        Living::new(placement.yaw),
        Equipment::default(),
    ))
}
