use crate::world::block::behaviour::{BlockBehaviour, Properties};
use bevy_ecs::prelude::Resource;
use mcrs_protocol::{BlockStateId, Ident};
use mcrs_registry::RegistryEntry;
use std::hash::{Hash, Hasher};

pub mod behaviour;
pub mod minecraft;

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct BlockUpdateFlags: u32 {
        const NEIGHBORS = 1;
        const CLIENTS   = 2;
        const INVISIBLE = 4;
        const IMMEDIATE = 8;
        const KNOWN_SHAPE = 16;
        const SUPPRESS_DROPS = 32;
        const MOVE_BY_PISTON = 64;
        const SKIP_SHAPE_UPDATE_ON_WIRE = 128;
        const SKIP_BLOCK_ENTITY_SIDEEFFECTS = 256;
        const SKIP_ON_PLACE = 512;
        const NONE = BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDEEFFECTS.bits() | BlockUpdateFlags::INVISIBLE.bits();
        const ALL = BlockUpdateFlags::NEIGHBORS.bits() | BlockUpdateFlags::CLIENTS.bits();
        const ALL_IMMEDIATE = BlockUpdateFlags::ALL.bits() | BlockUpdateFlags::IMMEDIATE.bits();
    }
}

#[derive(Debug)]
pub struct Block {
    pub identifier: Ident<&'static str>,
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
