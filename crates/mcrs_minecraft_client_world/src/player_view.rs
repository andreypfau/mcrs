use bevy::app::{App, Plugin};
use bevy::ecs::prelude::{Commands, Component, On};
use bevy::log::warn;
use bevy::math::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundPlayerPosition, ClientboundSetChunkCacheCenter,
};

/// Keeps on the connection where the server puts the player and which columns it keeps
/// loaded around it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerViewPlugin;

impl Plugin for PlayerViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_player_position)
            .add_observer(on_set_chunk_cache_center)
            .add_observer(on_chunk_cache_radius);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ServerTeleport {
    pub teleport_id: i32,
    pub position: DVec3,
    pub velocity: DVec3,
    pub look: Look,
}

/// Teleports the server has sent that the player has yet to be moved to and
/// confirm. A queue rather than the latest one, because the server keeps a
/// count of outstanding confirmations and a dropped teleport never unblocks.
#[derive(Component, Default, Debug)]
pub struct PendingTeleports(pub Vec<ServerTeleport>);

/// The column the server centres the client's loaded area on.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheCenter(pub ColumnPos);

/// The radius in columns of the area the server keeps loaded for the client.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheRadius(pub i32);

fn on_player_position(event: On<Received<ClientboundPlayerPosition>>, mut commands: Commands) {
    let Some(position) = event.packet() else {
        return;
    };
    if !position.flags.is_empty() {
        // chisle: every relative flag is treated as absolute. Our server
        // only ever sends absolute teleports; applying them means adding
        // each flagged component to the current one.
        warn!(
            "relative teleport flags are not applied: {:?}",
            position.flags
        );
    }
    let teleport = ServerTeleport {
        teleport_id: position.teleport_id.0,
        position: position.position,
        velocity: position.velocity,
        look: position.look,
    };
    commands
        .entity(event.entity)
        .entry::<PendingTeleports>()
        .or_default()
        .and_modify(move |mut pending| pending.0.push(teleport));
}

fn on_set_chunk_cache_center(
    event: On<Received<ClientboundSetChunkCacheCenter>>,
    mut commands: Commands,
) {
    let Some(center) = event.packet() else {
        return;
    };
    commands
        .entity(event.entity)
        .insert(ChunkCacheCenter(ColumnPos::new(center.x.0, center.z.0)));
}

fn on_chunk_cache_radius(event: On<Received<ClientboundChunkCacheRadius>>, mut commands: Commands) {
    let Some(radius) = event.packet() else {
        return;
    };
    commands
        .entity(event.entity)
        .insert(ChunkCacheRadius(radius.radius.0));
}
