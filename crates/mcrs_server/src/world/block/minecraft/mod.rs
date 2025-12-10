use bevy_app::{App, Plugin};
use mcrs_protocol::BlockStateId;
use mcrs_registry::RegistryEntry;
use crate::world::block::behaviour::Properties;

pub mod tnt;

#[derive(Debug, Clone, Default)]
pub struct Block {
    properties: Properties,
    default_state: BlockStateId,
}

impl Block {
    pub const fn new(properties: Properties, default_state: BlockStateId) -> Self {
        Self {
            properties,
            default_state,
        }
    }
}

impl From<&Block> for BlockStateId {
    fn from(value: &Block) -> Self {
        value.default_state
    }
}

pub static AIR: Block = Block::new(
    Properties::new()
        .replacable()
        .no_collision()
        .no_loot_table()
        .air(),
    BlockStateId(0),
);

pub static STONE: Block = Block::new(
    Properties::new()
        .requires_correct_tool_for_drops()
        .destroy_time(1.5)
        .explosion_resistance(6.0),
    BlockStateId(1)
);

pub static TNT: Block = Block::new(tnt::PROPERTIES, tnt::DEFAULT_STATE);

pub struct MinecraftBlockPlugin;

impl Plugin for MinecraftBlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(tnt::TntBlockPlugin);
    }
}