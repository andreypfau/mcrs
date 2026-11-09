use crate::Intents;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;

/// The result of one full evaluation of one mob in the AI stage. It writes only the guidance state
/// of its own mob.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub mob: EntityId,
    pub key: EntityKey,
    pub issue_tick: u64,
    pub intents: Intents,
}
