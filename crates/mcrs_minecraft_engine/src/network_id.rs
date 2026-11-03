//! The ids of entities on the wire.

use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};

/// The id of a player on the wire. It comes from the session, which the player keeps in every
/// dimension. Players take the odd ids and every other entity the even ones, so the
/// ids of two allocators never meet.
pub fn player_network_id(session: PlayerSession) -> i32 {
    // Sessions count up from one, so the low 31 bits of live sessions only meet after 2^31
    // logins.
    let low = session.0 as u32;
    (low.wrapping_mul(2) | 1) as i32
}

/// The id on the wire of `entity`: a player's from its session, any other entity's from
/// itself.
pub fn network_id(entity: Entity, owner: Option<&Owner>) -> i32 {
    owner.map_or_else(
        || entity_network_id(entity),
        |owner| player_network_id(owner.0),
    )
}

/// The id on the wire of an entity that is not a player: a new one in each dimension it enters.
fn entity_network_id(entity: Entity) -> i32 {
    entity.index_u32().wrapping_mul(2) as i32
}
