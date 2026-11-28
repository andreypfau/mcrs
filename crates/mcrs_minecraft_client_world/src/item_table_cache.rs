use bevy::prelude::Resource;
use mcrs_minecraft_engine_registries::ItemTableInputs;
use mcrs_minecraft_item_table::ItemTable;

#[derive(Resource, Default)]
pub(crate) struct ItemTableCache(pub(crate) Option<BakedItemTable>);

pub(crate) struct BakedItemTable {
    pub(crate) inputs: ItemTableInputs,
    pub(crate) table: ItemTable,
}
