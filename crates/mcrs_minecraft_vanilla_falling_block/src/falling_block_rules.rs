use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_falling_block::ConcreteTable;
use mcrs_minecraft_falling_block::FallingBlockTable;
use mcrs_minecraft_falling_block::PackPairs;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::BlockStateId;

use crate::PackConcretePairs;

/// What the plugin was built with.
#[derive(Resource, Clone, Debug)]
pub(crate) struct FallingBlockConfig {
    pub(crate) settings: Settings,
    pub(crate) concrete_pairs: Vec<PackConcretePairs>,
}

/// The tables of the falling block, from the block definitions, the tags and the concrete pairs.
/// Without it the plugin does nothing.
#[derive(Resource, Debug)]
pub(crate) struct FallingBlockRules {
    pub(crate) table: FallingBlockTable,
    pub(crate) settings: Settings,
    /// The item of each block, by block index: the first item that places it.
    items: Box<[Option<Kind>]>,
}

impl FallingBlockRules {
    /// The item of the block of `state`, or `None` for a block with no item.
    pub(crate) fn item_of(
        &self,
        definitions: &BlockDefinitions,
        state: BlockStateId,
    ) -> Option<Kind> {
        let block = definitions.block_index(state)?;
        self.items.get(block.index()).copied().flatten()
    }
}

pub(crate) fn build_rules(
    mut commands: Commands,
    config: Res<FallingBlockConfig>,
    blocks: Res<Blocks>,
    items: Res<Items>,
    registries: Res<CurrentRegistries>,
) {
    let (Some(block_tags), Some(fluid_tags)) =
        (registries.tags::<Block>(), registries.tags::<Fluid>())
    else {
        tracing::error!("the registries declare no block or fluid tags; nothing falls");
        return;
    };
    let packs = config.concrete_pairs.iter().map(|pairs| PackPairs {
        pack: &pairs.pack,
        file: &pairs.file,
    });
    let concrete = match ConcreteTable::load(packs, &blocks) {
        Ok(concrete) => concrete,
        Err(error) => {
            tracing::error!("the concrete pairs do not load, so nothing falls: {error}");
            return;
        }
    };
    let mut block_items: Box<[Option<Kind>]> = vec![None; blocks.blocks().len()].into_boxed_slice();
    for item in items.iter() {
        let Some(block) = item
            .block_placer
            .and_then(|state| blocks.block_index(state))
        else {
            continue;
        };
        if let Some(slot @ None) = block_items.get_mut(block.index()) {
            *slot = Some(Kind(item.id.number()));
        }
    }
    commands.insert_resource(FallingBlockRules {
        table: FallingBlockTable::new(&blocks, &block_tags, &fluid_tags, &concrete),
        settings: config.settings,
        items: block_items,
    });
}
