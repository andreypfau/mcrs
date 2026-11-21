use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_redstone::RedstoneTable;
use mcrs_minecraft_registries::Registries;

#[must_use]
pub fn redstone_table(definitions: &BlockDefinitions, registries: &Registries) -> RedstoneTable {
    let tags = registries.tags::<Block>();
    RedstoneTable::new(definitions, |block, tag| {
        tags.as_ref()
            .is_some_and(|tags| tags.id(tag).is_some_and(|tag| tags.contains(tag, block)))
    })
}
