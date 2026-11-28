use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::voxel_update::CellBlockWrite;
use mcrs_minecraft_simulation::{ChainLedger, Message};

use crate::world_kinds::WorldKinds;

#[derive(Debug, Default)]
pub struct BlockMail {
    pub(crate) writes: Vec<CellBlockWrite>,
    pub(crate) messages: Vec<Message<WorldKinds>>,
    pub(crate) ledgers: Vec<ChainLedger>,
}

impl BlockMail {
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.writes.is_empty() && self.messages.is_empty() && self.ledgers.is_empty()
    }

    pub(crate) fn append(&mut self, other: &mut Self) {
        self.writes.append(&mut other.writes);
        self.messages.append(&mut other.messages);
        self.ledgers.append(&mut other.ledgers);
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct BlockOutbox(pub(crate) BlockMail);
