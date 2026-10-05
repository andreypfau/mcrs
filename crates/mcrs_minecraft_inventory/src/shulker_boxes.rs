use bevy_ecs::resource::Resource;
use mcrs_minecraft_keys::{Block, Item, block_tags, item_tags};
use mcrs_minecraft_registry::{Id, RegistrySet, TagId, Tags};

#[derive(Resource, Clone, Debug)]
pub struct ShulkerBoxes {
    items: Tags<Item>,
    item_tag: TagId<Item>,
    blocks: Tags<Block>,
    block_tag: TagId<Block>,
}

impl ShulkerBoxes {
    pub fn new(set: &RegistrySet) -> Option<Self> {
        let items = set.tags::<Item>()?;
        let blocks = set.tags::<Block>()?;
        Some(ShulkerBoxes {
            item_tag: items.get(&item_tags::SHULKER_BOXES)?,
            block_tag: blocks.get(&block_tags::SHULKER_BOXES)?,
            items,
            blocks,
        })
    }

    pub fn has_item(&self, item: Id<Item>) -> bool {
        self.items.contains(self.item_tag, item)
    }

    pub fn has_block(&self, block: Id<Block>) -> bool {
        self.blocks.contains(self.block_tag, block)
    }
}
