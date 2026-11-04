//! The ids of entities on the wire.

use mcrs_minecraft_engine_world::session::PlayerSession;

/// The id of a player on the wire. It comes from the session, which the player keeps in every
/// dimension. Players take the odd ids and every other entity the even ones, so the
/// ids of two allocators never meet.
pub fn player_network_id(session: PlayerSession) -> i32 {
    // Sessions count up from one, so the low 31 bits of live sessions only meet after 2^31
    // logins.
    let low = session.0 as u32;
    (low.wrapping_mul(2) | 1) as i32
}
