use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

/// The values one evaluation of a block table reads (LOOT-CTX-001). `block_at` reads the world
/// at the time of the evaluation.
pub struct LootContext<'a, W> {
    pub origin: BlockPos,
    pub state: BlockStateId,
    pub tool: Tool<'a>,
    pub this_entity: bool,
    pub explosion_radius: Option<f32>,
    pub block_at: W,
}

/// The stack of the context value `tool`; no item is the empty stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tool<'a> {
    pub item: Option<Id<Item>>,
    pub enchantments: &'a [(Id<Enchantment>, u32)],
}

impl Tool<'_> {
    pub(crate) fn level(&self, enchantment: Option<Id<Enchantment>>) -> u32 {
        self.enchantments
            .iter()
            .find(|(id, _)| Some(*id) == enchantment)
            .map_or(0, |&(_, level)| level)
    }
}
