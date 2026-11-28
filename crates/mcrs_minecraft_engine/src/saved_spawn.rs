use bevy_ecs::resource::Resource;
use mcrs_minecraft_save::RespawnData;

#[derive(Resource, Debug, Clone)]
pub struct SavedSpawn(pub RespawnData);
