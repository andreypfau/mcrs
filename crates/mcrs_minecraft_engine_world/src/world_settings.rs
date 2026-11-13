use bevy_ecs::prelude::Resource;
use mcrs_minecraft_entity::{Difficulty as WorldDifficulty, WorldBorderSettings as BorderSettings};

pub use mcrs_minecraft_entity::Difficulty as EntityDifficulty;

/// The difficulty of a world, owned by the engine so that every damage source reads one value.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Difficulty(pub WorldDifficulty);

/// The border of a world, owned by the engine so that every damage source reads one value.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldBorderSettings(pub BorderSettings);
