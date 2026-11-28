use bevy::prelude::*;
use mcrs_minecraft_client_world::player::Player;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldPresence {
    NoWorld,
    InWorld,
}

pub(crate) fn world_presence(players: &Query<(), With<Player>>) -> WorldPresence {
    if players.is_empty() {
        WorldPresence::NoWorld
    } else {
        WorldPresence::InWorld
    }
}
