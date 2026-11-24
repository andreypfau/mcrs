use bevy_app::{App, Plugin, SubApp};
use bevy_ecs::entity::Entity;
use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_support::BlockSupport;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine::block_simulation::{
    BlockSimulationPlugin, DimensionBlockRules, InertBlocks,
};
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_world::block_drops::{BlockDrops, DroppingBlock};
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, ItemStack as StackValue, Kind};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{LootContext, Tool};
use mcrs_minecraft_loot_table::{KeyedSeed, LootScratch, LootTables};
use mcrs_minecraft_predicate::BlockHost;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{BlockBehaviour, BlockFacts, Cx, SimulationVolume};
use mcrs_voxel::{BlockPos, BlockStateId};

pub struct VanillaBlocksPlugin;

impl Plugin for VanillaBlocksPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DimensionBlockRules(add_loot_rules));
    }
}

#[derive(Clone, Debug)]
pub struct VanillaBlocks {
    loot: LootTables,
    item_count: usize,
    support: BlockSupport,
    blocks: Blocks,
    air: BlockStateId,
    void_air: BlockStateId,
}

impl BlockBehaviour<WorldKinds> for VanillaBlocks {
    fn update_shape<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        from: Direction,
        _neighbour: BlockPos,
        _neighbour_state: BlockStateId,
    ) -> BlockStateId {
        let mut unavailable = false;
        let read = |pos| {
            cx.state(pos).unwrap_or_else(|_| {
                unavailable = true;
                self.void_air
            })
        };
        if self.support.stays_after_update(state, at, from, read) || unavailable {
            state
        } else {
            self.air
        }
    }
}

impl BlockDrops for VanillaBlocks {
    fn drops(
        &self,
        block: &DroppingBlock,
        block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
        drops: &mut Vec<ItemStack>,
    ) {
        let table = self
            .blocks
            .state(block.state)
            .loot
            .and_then(|loot| self.loot.block_table(loot));
        let Some(table) = table else {
            return;
        };
        let tool = block.tool.and_then(|stack| {
            Some(StackValue::new(
                Kind(stack.item.number()),
                Count::new(stack.count)?,
            ))
        });
        let Some(mut context) = LootContext::try_new(ContextKeySet::Block) else {
            return;
        };
        context.origin = Some(DVec3::new(
            f64::from(block.at.x) + 0.5,
            f64::from(block.at.y) + 0.5,
            f64::from(block.at.z) + 0.5,
        ));
        context.block_state = Some(block.state);
        context.tool = Some(tool.as_ref().map_or(Tool::Empty, Tool::Stack));
        context.this_entity = block.breaker;
        let random = self.loot.keyed_stream(KeyedSeed {
            world: block.world_seed,
            table,
            origin: block.at,
            tick: block.tick,
            cause: match block.breaker {
                Some(_) => BREAK,
                None => REMOVAL,
            },
        });
        let mut stacks = Vec::new();
        let rolled = random.and_then(|mut random| {
            self.loot.roll(
                table,
                &context,
                &BlockHost::<_, Entity>::new(block_at),
                &mut random,
                &mut LootScratch::new(),
                &mut stacks,
            )
        });
        if let Err(error) = rolled {
            tracing::warn!("a block table did not roll: {error}");
            return;
        }
        drops.extend(stacks.into_iter().flatten().filter_map(|stack| {
            let item = Id::<Item>::checked(stack.kind.0, self.item_count)?;
            Some(ItemStack::new(item, stack.count.get()))
        }));
    }
}

const BREAK: u64 = 1;
const REMOVAL: u64 = 2;

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
    let (loot, issues) = LootTables::bake(blocks, registries, items);
    for issue in &issues {
        tracing::error!("{issue}");
    }
    let (support, issues) = BlockSupport::bake(blocks, registries);
    for issue in &issues {
        tracing::warn!("{issue}");
    }
    let rules = VanillaBlocks {
        loot,
        item_count: registries.entries::<Item>().map_or(0, |items| items.len()),
        support: support.clone(),
        air: blocks.default_state_of(Block::Air.id()),
        void_air: blocks.default_state_of(Block::VoidAir.id()),
        blocks: blocks.clone(),
    };
    sub_app.world_mut().insert_resource(support);
    sub_app.add_plugins(BlockSimulationPlugin::new(rules));
}
