use bevy::prelude::*;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;

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
