use std::net::SocketAddr;

use bevy_app::App;
use bevy_app::Plugin;
use bevy_app::Startup;
use bevy_ecs::prelude::*;
use mcrs_minecraft_network::client::JoinRequest;

const VIEW_DISTANCE: u8 = 2;

/// Joins `server` once at startup, as an offline player.
#[derive(Resource, Clone, Debug)]
pub struct JoinServerPlugin {
    pub server: SocketAddr,
    pub username: String,
}

impl Plugin for JoinServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.clone()).add_systems(Startup, join);
    }
}

fn join(settings: Res<JoinServerPlugin>, mut joins: MessageWriter<JoinRequest>) {
    joins.write(JoinRequest {
        server: settings.server,
        username: settings.username.clone(),
        profile_id: None,
        view_distance: VIEW_DISTANCE,
    });
}
