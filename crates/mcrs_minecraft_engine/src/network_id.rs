use crate::session::HostAnchor;
use bevy_ecs::entity::Entity;

/// The id of a player on the wire. It comes from the host anchor, which the player keeps in every
/// dimension (DIM-CHG-001). Players take the odd ids and every other entity the even ones, so the
/// ids of two allocators never meet.
pub fn player_network_id(anchor: Entity) -> i32 {
    (anchor.index_u32().wrapping_mul(2) | 1) as i32
}

/// The id on the wire of `entity`: a player's from its host anchor, any other entity's from
/// itself.
pub fn network_id(entity: Entity, anchor: Option<&HostAnchor>) -> i32 {
    anchor.map_or_else(
        || entity_network_id(entity),
        |anchor| player_network_id(anchor.0),
    )
}

/// The id on the wire of an entity that is not a player: a new one in each dimension it enters
/// (DIM-CHG-009).
fn entity_network_id(entity: Entity) -> i32 {
    entity.index_u32().wrapping_mul(2) as i32
}
