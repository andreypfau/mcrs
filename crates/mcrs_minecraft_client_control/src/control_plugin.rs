use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use mcrs_minecraft_client_world::hotbar_sync::SendSelectedSlot;
use mcrs_minecraft_client_world::local_player::LocalPlayerTick;

use crate::control;
use crate::control::Control;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            control::serve
                .before(SendSelectedSlot)
                .before(LocalPlayerTick)
                .run_if(resource_exists::<Control>),
        );
    }
}
