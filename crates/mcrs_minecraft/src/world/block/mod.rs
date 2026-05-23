use crate::world::block::behaviour::Properties;
use mcrs_protocol::{BlockStateId, Ident};
use std::hash::{Hash, Hasher};

pub mod behaviour;
mod macros;
pub mod minecraft;

#[derive(Debug)]
pub struct Block {
    pub identifier: Ident<&'static str>,
    /// Vanilla `minecraft:block` registry index (protocol ID).
    /// Must match the client's built-in registry ordering.
    pub protocol_id: u16,
    pub properties: &'static Properties,
    pub default_state: &'static BlockState,
    pub states: &'static [BlockState],
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identifier.hash(state);
    }
}

impl Block {
    #[inline]
    pub fn hardness(&self) -> f32 {
        self.properties.hardness
    }

    pub fn explosion_resistance(&self) -> f32 {
        self.properties.explosion_resistance
    }

    pub fn requires_correct_tool_for_drops(&self) -> bool {
        self.properties.requires_correct_tool_for_drops
    }

    pub fn xp_range(&self) -> Option<(u32, u32)> {
        self.properties.xp_range
    }
}

impl From<&'static Block> for BlockStateId {
    fn from(block: &'static Block) -> Self {
        block.default_state.id
    }
}

#[derive(Debug, Eq)]
pub struct BlockState {
    pub id: BlockStateId,
}

impl PartialEq for BlockState {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<BlockState> for BlockStateId {
    fn from(state: BlockState) -> Self {
        state.id
    }
}
