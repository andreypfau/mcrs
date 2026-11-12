use bevy_ecs::prelude::Component;

/// The on-ground flag of the last movement packet of the player's client that the server applied.
/// False until the first one; a vehicle move does not change it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClientOnGround(pub bool);
