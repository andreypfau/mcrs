use mcrs_minecraft_mob::MobType;
use uuid::Uuid;

use crate::mob_looks::MobLooks;
use crate::mob_state::MobState;

/// What a world publishes when a mob comes into it.
#[derive(Clone, Debug, PartialEq)]
pub struct MobSpawn {
    pub kind: MobType,
    pub uuid: Uuid,
    /// The state when the server starts to track the mob, which the spawn bundle shows.
    pub state: MobState,
    pub looks: MobLooks,
}
