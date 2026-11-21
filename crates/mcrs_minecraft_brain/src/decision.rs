use crate::Intents;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub mob: EntityId,
    pub key: EntityUuid,
    pub issue_tick: u64,
    pub intents: Intents,
}
