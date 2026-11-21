use mcrs_minecraft_entity_key::EntityUuid;

use crate::YzxPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WriterKey {
    Command { sequence: u64 },
    Entity { key: EntityUuid },
    Block { pos: YzxPos },
}
