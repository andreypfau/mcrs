use mcrs_minecraft_protocol::packets::table::game_serverbound::NAMES;
use mcrs_minecraft_protocol::packets::table::row_of;

/// Evaluated in a `const` item, so a name outside the play table fails the build.
const fn id(name: &str) -> i32 {
    let row = row_of(NAMES, name);
    assert!(row.is_some(), "the name is not a row of the play table");
    match row {
        Some(id) => id,
        None => -1,
    }
}

pub(crate) const ACCEPT_TELEPORTATION: i32 = id("accept_teleportation");
pub(crate) const CLIENT_TICK_END: i32 = id("client_tick_end");
pub(crate) const MOVE_PLAYER_POS: i32 = id("move_player_pos");
pub(crate) const MOVE_PLAYER_POS_ROT: i32 = id("move_player_pos_rot");
pub(crate) const MOVE_PLAYER_ROT: i32 = id("move_player_rot");
pub(crate) const MOVE_PLAYER_STATUS_ONLY: i32 = id("move_player_status_only");
pub(crate) const MOVE_VEHICLE: i32 = id("move_vehicle");
pub(crate) const PLAYER_ABILITIES: i32 = id("player_abilities");
pub(crate) const PLAYER_COMMAND: i32 = id("player_command");
pub(crate) const PLAYER_INPUT: i32 = id("player_input");
pub(crate) const PLAYER_LOADED: i32 = id("player_loaded");
pub(crate) const PONG: i32 = id("pong");
