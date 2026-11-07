//! Whether the server checks the moves of its players.

use bevy_ecs::resource::Resource;

/// Whether the server checks the moves of its players, as `docs/spec/movement-checks.md` states.
/// The host holds it, and each dimension world takes a copy when it spawns.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MovementChecks {
    /// An accept confirms a teleport only at its destination, and its position never becomes the
    /// position of the player.
    #[default]
    On,
    /// An accept that confirms a teleport is also a move from the destination to the position and
    /// the rotation of the accept.
    Off,
}
