use mcrs_minecraft_entity_key::EntityId;

use crate::VisibleState;

/// A living row whose state that the viewers see changed in the pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moved {
    pub id: EntityId,
    pub state: VisibleState,
}
