use core::iter;

use bevy_app::{App, Plugin, SubApp};
use bevy_ecs::entity::Entity;
use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_support::BlockSupport;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine::block_simulation::{
    BlockSimulationConfig, BlockSimulationPlugin, DimensionBlockRules, InertBlocks,
};
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_world::block_drops::{BlockDrops, DroppingBlock};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::{AttributeValue, WorldClock};
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, ItemStack as StackValue, Kind};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{
    ClockView, EnvironmentView, LootContext, NbtNumber, ScoreHolder, ScoreView, StorageView, Tool,
    WeatherView,
};
use mcrs_minecraft_loot_table::{KeyedSeed, LootScratch, LootTables};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_predicate::{ActiveEffect, EntityView, ItemSubject, LocationView};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::{Id, ResourceLocation};
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, ChainLimit, Cx, KernelConfig, SimulationVolume,
};
use mcrs_voxel::{BlockPos, BlockStateId};

pub struct VanillaBlocksPlugin {
    pub chain_limit: ChainLimit,
}

impl Default for VanillaBlocksPlugin {
    fn default() -> Self {
        Self {
            chain_limit: KernelConfig::default().chain_limit,
        }
    }
}

impl Plugin for VanillaBlocksPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BlockSimulationConfig(KernelConfig {
            chain_limit: self.chain_limit,
            ..KernelConfig::default()
        }));
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
                &BlockAt(block_at),
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

struct BlockAt<F>(F);

impl<F: Fn(BlockPos) -> Option<BlockStateId>> LocationView for BlockAt<F> {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId> {
        (self.0)(pos)
    }

    fn is_loaded(&self, pos: BlockPos) -> bool {
        (self.0)(pos).is_some()
    }

    fn fluid_state(&self, _: BlockPos) -> Option<mcrs_minecraft_block::definition::FluidState> {
        None
    }

    fn block_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light_level(&self) -> Option<f32> {
        None
    }

    fn biome(
        &self,
        _: BlockPos,
    ) -> Option<mcrs_minecraft_resource::Id<mcrs_minecraft_biome::Biome>> {
        None
    }

    fn inside_structure(
        &self,
        _: BlockPos,
        _: mcrs_minecraft_resource::Id<mcrs_minecraft_entity::spawn_condition::Structure>,
    ) -> Option<bool> {
        None
    }

    fn dimension(
        &self,
    ) -> Option<mcrs_minecraft_resource::Id<mcrs_minecraft_dimension::Dimension>> {
        None
    }

    fn block_entity_data(
        &self,
        _: BlockPos,
        _: &mut mcrs_minecraft_nbt::compound::NbtCompound,
    ) -> bool {
        false
    }
}

impl<F> EntityView for BlockAt<F> {
    type Entity = Entity;

    fn entity_type(&self, _: Entity) -> Option<Id<EntityType>> {
        None
    }

    fn position(&self, _: Entity) -> Option<DVec3> {
        None
    }

    fn is_on_ground(&self, _: Entity) -> Option<bool> {
        None
    }

    fn support_position(&self, _: Entity) -> Option<BlockPos> {
        None
    }

    fn movement_position(&self, _: Entity) -> Option<BlockPos> {
        None
    }

    fn reported_movement(&self, _: Entity) -> Option<DVec3> {
        None
    }

    fn fall_distance(&self, _: Entity) -> Option<f64> {
        None
    }

    fn saved_data(&self, _: Entity, _: &mut NbtCompound) -> bool {
        false
    }

    fn is_burning(&self, _: Entity) -> Option<bool> {
        None
    }

    fn pose(&self, _: Entity) -> Option<Pose> {
        None
    }

    fn is_sprinting(&self, _: Entity) -> Option<bool> {
        None
    }

    fn is_swimming(&self, _: Entity) -> Option<bool> {
        None
    }

    fn is_in_water(&self, _: Entity) -> Option<bool> {
        None
    }

    fn is_living(&self, _: Entity) -> Option<bool> {
        None
    }

    fn is_fall_flying(&self, _: Entity) -> Option<bool> {
        None
    }

    fn has_flying_ability(&self, _: Entity) -> Option<bool> {
        None
    }

    fn is_baby(&self, _: Entity) -> Option<bool> {
        None
    }

    fn active_effect(&self, _: Entity, _: Id<MobEffect>) -> Option<ActiveEffect> {
        None
    }

    fn equipment(&self, _: Entity, _: EquipmentSlot) -> Option<ItemSubject<'_>> {
        None
    }

    fn slot(&self, _: Entity, _: u16) -> Option<ItemSubject<'_>> {
        None
    }

    fn tick_count(&self, _: Entity) -> Option<u32> {
        None
    }

    fn vehicle(&self, _: Entity) -> Option<Entity> {
        None
    }

    fn passengers(&self, _: Entity) -> Option<impl Iterator<Item = Entity>> {
        None::<iter::Empty<Entity>>
    }

    fn attack_target(&self, _: Entity) -> Option<Entity> {
        None
    }

    fn team(&self, _: Entity) -> Option<&str> {
        None
    }

    fn has_scoreboard_tag(&self, _: Entity, _: &str) -> Option<bool> {
        None
    }
}

impl<F, E> ScoreView<E> for BlockAt<F> {
    fn score(&self, _: ScoreHolder<'_, E>, _: &str) -> Option<i32> {
        None
    }
}

impl<F> StorageView for BlockAt<F> {
    fn storage_number(&self, _: ResourceLocation<'_>, _: &NbtPath) -> Option<NbtNumber> {
        None
    }
}

impl<F> EnvironmentView for BlockAt<F> {
    fn environment_value(
        &self,
        _: EnvironmentAttribute,
        _: Option<DVec3>,
    ) -> Option<AttributeValue> {
        None
    }
}

impl<F> WeatherView for BlockAt<F> {
    fn rain_strength(&self) -> Option<f32> {
        None
    }

    fn thunder_strength(&self) -> Option<f32> {
        None
    }
}

impl<F> ClockView for BlockAt<F> {
    fn total_ticks(&self, _: Id<WorldClock>) -> Option<i64> {
        None
    }
}

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
