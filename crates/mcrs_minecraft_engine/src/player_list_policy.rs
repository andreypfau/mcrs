use bevy_ecs::resource::Resource;
use mcrs_minecraft_replication::ListPolicy;

/// Chooses the players that the player list of each client shows. Without this resource, the
/// list shows the nearest players.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct PlayerListPolicy(pub ListPolicy);
