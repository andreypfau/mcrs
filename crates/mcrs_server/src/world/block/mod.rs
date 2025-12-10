use crate::world::block::behaviour::BlockBehaviour;
use crate::world::block::minecraft::Block;
use bevy_ecs::prelude::Resource;
use mcrs_protocol::ident;
use mcrs_registry::{Registry, RegistryEntry};
use std::sync::Arc;

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
        // остальные по надобности
    }
}

#[derive(Resource)]
struct BlockRegistry {
    registry: Registry<BlockEntry>,
}

struct BlockEntry {
    block: Arc<&'static Block>,
}

impl AsRef<Block> for BlockEntry {
    fn as_ref(&self) -> &Block {
        self.block.as_ref()
    }
}

impl RegistryEntry for BlockEntry {}

impl From<&'static Block> for BlockEntry {
    fn from(block: &'static Block) -> Self {
        Self {
            block: Arc::new(block),
        }
    }
}

impl BlockRegistry {
    fn new() -> Self {
        let mut registry = Registry::new();
        registry.insert(ident!("air"), (&minecraft::AIR).into());
        registry.insert(ident!("stone"), (&minecraft::STONE).into());
        registry.insert(ident!("tnt"), (&minecraft::TNT).into());

        Self { registry }
    }
}
