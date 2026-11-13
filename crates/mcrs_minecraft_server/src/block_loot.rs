use bevy_app::{App, Plugin, SubApp};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::block_simulation::{
    BlockSimulationPlugin, DimensionBlockRules, InertBlocks,
};
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_world::block_drops::{BlockDrops, DroppingBlock};
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_loot_table::{BlockLoot, LootCause, LootContext, LootSeed, Tool};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_simulation::BlockBehaviour;
use mcrs_voxel::{BlockPos, BlockStateId};

/// The drops of the blocks in every dimension world, from the block tables of the loaded data
/// pack. No block reacts otherwise, as under [`InertBlocks`].
pub struct VanillaBlockLootPlugin;

impl Plugin for VanillaBlockLootPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DimensionBlockRules(add_loot_rules));
    }
}

/// Block rules whose drops are the block tables (`block-loot.md`).
#[derive(Clone, Debug)]
pub struct LootBlocks {
    loot: BlockLoot,
    blocks: Blocks,
}

impl BlockBehaviour<WorldKinds> for LootBlocks {}

impl BlockDrops for LootBlocks {
    fn drops(
        &self,
        block: &DroppingBlock,
        block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
        drops: &mut Vec<ItemStack>,
    ) {
        let Some(table) = self.blocks.state(block.state).loot else {
            return;
        };
        // chisle: a stack holds no enchantments, so each enchantment of the tool has the level 0.
        let cx = LootContext {
            origin: block.at,
            state: block.state,
            tool: Tool {
                item: block.tool.map(|stack| stack.item),
                enchantments: &[],
            },
            this_entity: block.breaker.is_some(),
            explosion_radius: None,
            block_at,
        };
        let seed = LootSeed {
            world: block.world_seed,
            tick: block.tick,
            cause: match block.breaker {
                Some(_) => LootCause::Break,
                None => LootCause::Removal,
            },
        };
        let mut stacks = Vec::new();
        self.loot.drops(table, &cx, seed, &mut stacks);
        drops.extend(
            stacks
                .into_iter()
                .map(|stack| ItemStack::new(stack.item, stack.count)),
        );
    }
}

// chisle: the tables are baked once per dimension world at its spawn, so a reload of the data
// pack reaches only the worlds spawned after it; rebaking on a new registry snapshot lifts it.
fn add_loot_rules(sub_app: &mut SubApp) {
    let world = sub_app.world();
    let parts = (
        world.get_resource::<Blocks>(),
        world.get_resource::<CurrentRegistries>(),
        world.get_resource::<ItemTable>(),
    );
    let (Some(blocks), Some(registries), Some(items)) = parts else {
        tracing::warn!("the dimension world has no registries; its blocks drop nothing");
        sub_app.add_plugins(BlockSimulationPlugin::new(InertBlocks));
        return;
    };
    let (loot, issues) = BlockLoot::bake(blocks, registries, items);
    for issue in &issues {
        tracing::warn!("{issue}");
    }
    let rules = LootBlocks {
        loot,
        blocks: blocks.clone(),
    };
    sub_app.add_plugins(BlockSimulationPlugin::new(rules));
}
