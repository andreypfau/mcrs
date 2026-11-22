use bevy::prelude::*;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;

use crate::player::Player;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HardcoreMode {
    #[default]
    Off,
    On,
}

impl HardcoreMode {
    #[must_use]
    pub fn from_login(login: &ClientboundLogin) -> Self {
        if login.hardcore { Self::On } else { Self::Off }
    }
}

pub(crate) fn on_login(
    event: On<Received<ClientboundLogin>>,
    player: Query<Entity, With<Player>>,
    mut commands: Commands,
) {
    if let Some(login) = event.packet()
        && let Ok(player) = player.single()
    {
        commands
            .entity(player)
            .insert(HardcoreMode::from_login(&login));
    }
}
