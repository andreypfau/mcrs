use std::path::PathBuf;

use bevy_ecs::resource::Resource;

#[derive(Resource, Clone, Debug)]
pub struct WorldSave(pub PathBuf);
