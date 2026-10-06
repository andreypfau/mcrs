use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::item::Enchantments;
use mcrs_minecraft_registry::{BlockStateId, Tags};

pub struct BlockBreakContext<'a> {
    pub blocks: &'a BlockDefinitions,
    pub state: BlockStateId,
    pub tags: &'a Tags<Block>,
    pub tool_enchantments: Option<&'a Enchantments>,
}

#[derive(Debug, Clone)]
pub struct LootDrop {
    pub item_name: ResourceLocation,
    pub count: u8,
}
