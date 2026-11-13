use bevy_ecs::bundle::Bundle;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use uuid::Uuid;

use crate::client_entity::ClientEntity;
use crate::equipment::Equipment;
use crate::living::Living;
use crate::motion::{Age, Motion};
use crate::path::PacketPath;
use crate::placement::Placement;
use crate::receive::SNAP_DISTANCE;
use crate::synced_data::SyncedData;

/// The network id of a client entity that the host adds. [`crate::NetworkEntities`] never holds
/// it, so no packet of a server names the entity.
pub const HOST_NETWORK_ID: i32 = i32::MIN;

/// The components of a living client entity that the host adds and moves with
/// [`move_entity`], with no server: it ticks, turns and walks as an entity that a server adds.
/// `None` for a type that the type table does not list as living.
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
        client,
        Motion::new(placement, placement.position),
        Age::default(),
        SyncedData::default(),
        Living::new(placement.yaw),
        Equipment::default(),
    ))
}

/// Moves an entity to `placement` over the update interval of its type, as a teleport of the
/// server does. A move longer than [`SNAP_DISTANCE`] puts the entity there at once.
pub fn move_entity(motion: &mut Motion, client: &ClientEntity, placement: Placement) {
    let position = placement.position;
    if motion.current.position.distance(position) > SNAP_DISTANCE {
        motion.snap(placement);
    } else {
        motion.start(
            Some(PacketPath::Linear(position)),
            Some(placement.rotation()),
            client.update_interval(),
        );
    }
}
