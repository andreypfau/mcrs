use std::io;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::error;
use bevy_log::info;
use mcrs_minecraft_client_world::hotbar_sync::SendSelectedSlot;
use mcrs_minecraft_client_world::local_player::LocalPlayerTick;

use crate::control;
use crate::control::Control;
use crate::listener;
use crate::listener::Listener;

pub struct ControlPlugin {
    listener: Listener,
    address: String,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlSet;

impl ControlPlugin {
    pub fn bind(address: &str) -> io::Result<Self> {
        Ok(Self {
            listener: listener::bind(address)?,
            address: address.to_owned(),
        })
    }
}

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        let listener = match self.listener.try_clone() {
            Ok(listener) => listener,
            Err(error) => {
                error!(address = self.address, %error, "the control socket did not open");
                return;
            }
        };
        info!(address = self.address, "control socket");
        app.insert_resource(Control::new(listener, self.address.clone()))
            .configure_sets(
                FixedUpdate,
                ControlSet.before(SendSelectedSlot).before(LocalPlayerTick),
            )
            .add_systems(FixedUpdate, control::serve.in_set(ControlSet));
    }
}
