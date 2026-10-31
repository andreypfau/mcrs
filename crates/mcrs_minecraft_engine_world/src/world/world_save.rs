use std::path::PathBuf;

use bevy_ecs::resource::Resource;

/// The world folder that saved chunks and players are read from.
#[derive(Resource, Clone, Debug)]
pub struct WorldSave(pub PathBuf);
