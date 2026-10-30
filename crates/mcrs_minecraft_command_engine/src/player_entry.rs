use bevy_ecs::entity::Entity;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::uuid::Uuid;

/// A player of any dimension, as a command finds it.
#[derive(Clone, Debug)]
pub struct PlayerEntry {
    /// The host anchor: the identity of the player in every dimension.
    pub anchor: Entity,
    /// The position of its dimension in [`CommandTargets::dimensions`](crate::CommandTargets::dimensions)(crate::CommandTargets::dimensions).
    pub dimension: usize,
    /// The entity of the player in the world of its dimension.
    pub entity: Entity,
    /// The UUID of the profile of the player.
    pub uuid: Uuid,
    /// The name of the profile of the player.
    pub name: String,
    /// The transform of the player, as the effects of the pass so far left it.
    pub transform: Transform,
    /// The game mode of the player, as the effects of the pass so far left it.
    pub mode: GameMode,
    /// The session id: sessions count up as players join, so it orders the player list, and a
    /// change of dimension keeps it.
    pub(crate) list_order: u64,
}
