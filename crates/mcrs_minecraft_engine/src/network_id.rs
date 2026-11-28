use mcrs_minecraft_engine_world::session::PlayerSession;

pub fn player_network_id(session: PlayerSession) -> i32 {
    let low = session.0 as u32;
    (low.wrapping_mul(2) | 1) as i32
}
