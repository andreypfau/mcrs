use bevy::prelude::*;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundGameEvent, ClientboundLogin};
use mcrs_minecraft_protocol_game::game_event::GameEventKind;

use crate::player::Player;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RespawnScreen {
    #[default]
    Shown,
    Immediate,
}

impl RespawnScreen {
    #[must_use]
    pub fn from_login(login: &ClientboundLogin) -> Self {
        if login.show_death_screen {
            Self::Shown
        } else {
            Self::Immediate
        }
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
            .insert(RespawnScreen::from_login(&login));
    }
}

pub(crate) fn on_game_event(
    event: On<Received<ClientboundGameEvent>>,
    mut screen: Query<&mut RespawnScreen, With<Player>>,
) {
    if let Some(packet) = event.packet()
        && let GameEventKind::ImmediateRespawn(immediate) = packet.game_event
        && let Ok(mut screen) = screen.single_mut()
    {
        screen.set_if_neq(if immediate {
            RespawnScreen::Immediate
        } else {
            RespawnScreen::Shown
        });
    }
}
