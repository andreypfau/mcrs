use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;

use crate::WriterKey;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyed<E> {
    pub target: EntityKey,
    pub target_id: EntityId,
    pub writer: WriterKey,
    pub sequence: u32,
    pub effect: E,
}
