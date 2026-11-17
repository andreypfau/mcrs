use mcrs_minecraft_entity::EntityTypeData;
use uuid::Uuid;

use crate::mob_state::MobState;
use crate::spawn_look::SpawnLook;

#[derive(Clone, Debug, PartialEq)]
pub struct MobSpawn {
    pub kind: &'static EntityTypeData,
    pub uuid: Uuid,
    pub state: MobState,
    pub look: SpawnLook,
}
