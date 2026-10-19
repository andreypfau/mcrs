use bevy_app::{App, Plugin};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::block_tags;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::TagId;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_item::keys::item_tags;
use mcrs_minecraft_registries::{Diagnostics, Registries, Tags};
use mcrs_minecraft_world::resolvers::{AddRegistryResolver, require_tag, require_tags};

pub struct InventoryIdsPlugin;

impl Plugin for InventoryIdsPlugin {
    fn build(&self, app: &mut App) {
        app.add_registry_resolver(ShulkerBoxes::resolve);
    }
}

#[derive(Debug)]
pub struct ShulkerBoxes {
    items: Tags<Item>,
    item_tag: TagId<Item>,
    blocks: Tags<Block>,
    block_tag: TagId<Block>,
}

impl ShulkerBoxes {
    pub fn resolve(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Self> {
        let items = require_tags(registries, mcrs_minecraft_item::keys::ITEM, diagnostics);
        let blocks = require_tags(registries, mcrs_minecraft_block::keys::BLOCK, diagnostics);
        let item_tag = items.as_ref().and_then(|tags| {
            require_tag(
                tags,
                mcrs_minecraft_item::keys::ITEM,
                &item_tags::SHULKER_BOXES,
                diagnostics,
            )
        });
        let block_tag = blocks.as_ref().and_then(|tags| {
            require_tag(
                tags,
                mcrs_minecraft_block::keys::BLOCK,
                &block_tags::SHULKER_BOXES,
                diagnostics,
            )
        });
        Some(ShulkerBoxes {
            item_tag: item_tag?,
            block_tag: block_tag?,
            items: items?,
            blocks: blocks?,
        })
    }

    pub fn has_item(&self, item: Id<Item>) -> bool {
        self.items.contains(self.item_tag, item)
    }

    pub fn has_block(&self, block: Id<Block>) -> bool {
        self.blocks.contains(self.block_tag, block)
    }
}
