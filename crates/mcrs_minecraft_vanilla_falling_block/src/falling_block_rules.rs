use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_falling_block::ConcreteTable;
use mcrs_minecraft_falling_block::FallingBlockTable;
use mcrs_minecraft_falling_block::PackConcretePairs;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::BlockStateId;

#[derive(Resource, Debug)]
pub(crate) struct FallingBlockRules {
    pub(crate) table: FallingBlockTable,
    pub(crate) settings: Settings,
    items: Box<[Option<Kind>]>,
}

impl FallingBlockRules {
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
    settings: Settings,
    concrete_pairs: &[PackConcretePairs],
    mut commands: Commands<'_, '_>,
    blocks: &Blocks,
    items: &Items,
    registries: &CurrentRegistries,
) {
    let (Some(block_tags), Some(fluid_tags)) =
        (registries.tags::<Block>(), registries.tags::<Fluid>())
    else {
        tracing::error!("the registries declare no block or fluid tags; nothing falls");
        return;
    };
    let concrete = match ConcreteTable::load(concrete_pairs, blocks) {
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
        table: FallingBlockTable::new(blocks, &block_tags, &fluid_tags, &concrete),
        settings,
        items: block_items,
    });
}
