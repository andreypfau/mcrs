use bevy_ecs::message::Message;
use glam::DVec3;
use mcrs_minecraft_entity::EntityTypeData;

#[derive(Message, Clone, Debug, PartialEq)]
pub struct MobSpawnRequest {
    pub kind: &'static EntityTypeData,
    pub position: DVec3,
    pub uuid: u128,
    pub setup: SpawnSetup,
    pub extras: MobExtras,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MobExtras {
    pub no_ai: bool,
    pub on_ground: bool,
    pub custom_name: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpawnSetup {
    First,
    None,
}
