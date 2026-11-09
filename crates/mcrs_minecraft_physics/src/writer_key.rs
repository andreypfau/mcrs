use mcrs_minecraft_entity_key::BirthKey;

use crate::YzxPos;

/// Who makes a write or a claim. The variants compare in the order `command < entity < block`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WriterKey {
    Command { sequence: u64 },
    Entity { key: BirthKey },
    Block { pos: YzxPos },
}
