use mcrs_minecraft_entity::EntityTypeData;
use uuid::Uuid;

use crate::mob_state::MobState;
use mcrs_minecraft_protocol_game::entity::MobLooks;

/// What a world publishes when a mob comes into it.
#[derive(Clone, Debug, PartialEq)]
pub struct MobSpawn {
    pub kind: &'static EntityTypeData,
    pub uuid: Uuid,
    /// The state when the server starts to track the mob, which the spawn bundle shows.
    pub state: MobState,
    pub looks: MobLooks,
}
