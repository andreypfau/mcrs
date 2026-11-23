use bevy::prelude::*;
use mcrs_minecraft_client_control::Meshed;

use crate::stream;

pub struct MeshedPlugin;

impl Plugin for MeshedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Meshed>().add_systems(
            Update,
            meshed
                .after(stream::flush_streams)
                .run_if(resource_exists::<stream::Loader>),
        );
    }
}

fn meshed(streaming: stream::Streaming, mut meshed: ResMut<Meshed>) {
    if !meshed.0 && streaming.done() && streaming.status().columns > 0 {
        meshed.0 = true;
    }
}
