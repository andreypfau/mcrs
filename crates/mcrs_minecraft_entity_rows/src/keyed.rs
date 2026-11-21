use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;

use crate::WriterKey;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyed<E> {
    pub target: EntityUuid,
    pub target_id: EntityId,
    pub writer: WriterKey,
    pub sequence: u32,
    pub effect: E,
}
