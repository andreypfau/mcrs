use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;

/// A player of any dimension, as a command finds it.
#[derive(Clone, Debug)]
pub struct PlayerEntry {
    /// The session: the identity of the player in every dimension.
    pub session: PlayerSession,
    /// The position of its dimension in [`CommandTargets::dimensions`](crate::CommandTargets::dimensions),
    /// as the effects so far left it.
    pub dimension: usize,
    /// The entity of the player in the world the command runs in; `None` when another world
    /// holds the player.
    pub entity: Option<Entity>,
    /// The UUID of the profile of the player.
    pub uuid: Uuid,
    /// The name of the profile of the player.
    pub name: String,
    /// The transform of the player, as the effects so far left it. A player of another world
    /// is where that world left it at the end of the last tick.
    pub transform: Transform,
    /// The game mode of the player, as the effects so far left it.
    pub mode: GameMode,
}
