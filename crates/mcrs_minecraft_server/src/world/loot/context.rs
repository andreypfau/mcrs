use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::item::Enchantments;

pub struct BlockBreakContext<'a> {
    pub tool_enchantments: Option<&'a Enchantments>,
}

#[derive(Debug, Clone)]
pub struct LootDrop {
    pub item_name: ResourceLocation,
    pub count: u8,
}
