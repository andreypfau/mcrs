use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_protocol::item::Enchantments;
use mcrs_minecraft_registry::{BlockStateId, Tags};

use crate::world::loot::LootRegistries;

pub struct BlockBreakContext<'a> {
    pub loot: &'a LootRegistries,
    pub blocks: &'a BlockDefinitions,
    pub state: BlockStateId,
    pub tags: &'a Tags<Block>,
    pub tool_enchantments: Option<&'a Enchantments>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootDrop {
    pub item: Item,
    pub count: u8,
}
