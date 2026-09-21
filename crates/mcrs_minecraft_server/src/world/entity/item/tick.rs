use crate::world::entity::item::{ITEM_HEIGHT, ITEM_WIDTH};
use bevy_ecs::entity::Entity;
use bevy_ecs::system::Command;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::{BlockStateFlags, Blocks};
use mcrs_minecraft_core::{BlockPos, LocalPos, SectionPos};
use mcrs_minecraft_inventory::{Op, Transaction};
use mcrs_minecraft_item::dropped::{
    AIR_DRAG, GRAVITY, INFINITE_LIFETIME, INFINITE_PICKUP_DELAY, LIFETIME,
};
use mcrs_minecraft_item::{
    DroppedItem, ItemStack, Items, max_stack_size, same_item_same_components,
};
use mcrs_minecraft_level::entity::physics::{Transform, Velocity};
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::lifecycle::level::SectionLevels;
use mcrs_minecraft_level::world::storage::section::SectionIndex;
use mcrs_minecraft_registry::BlockStateId;

const REST_SPEED_SQ: f64 = 1e-5;
const MERGE_RATE_MOVING: i32 = 2;
const MERGE_RATE_RESTING: i32 = 40;

struct Snapshot {
    entity: Entity,
    dim: Entity,
    pos: DVec3,
}

/// ponytail: vertical-only collision against full blocks, no water or lava
/// branches. Upgrade: an AABB sweep over the block shapes.
pub fn tick_dropped_items(world: &mut World) {
    let mut query = world.query::<(Entity, &InDimension, &Transform, &DroppedItem)>();
    let snapshot: Vec<Snapshot> = query
        .iter(world)
        .map(|(entity, dim, transform, _)| Snapshot {
            entity,
            dim: dim.0,
            pos: transform.translation,
        })
        .collect();
    if snapshot.is_empty() {
        return;
    }
    let blocks = world.resource::<Blocks>().clone();
    let items = world.resource::<Items>().clone();
    for current in &snapshot {
        let Some(item) = world.get::<DroppedItem>(current.entity).copied() else {
            continue;
        };
        if !world
            .get::<SectionLevels>(current.dim)
            .is_some_and(|levels| levels.is_entity_ticking(SectionPos::from(current.pos)))
        {
            continue;
        }
        let mut item = item;
        if item.pickup_delay > 0 && item.pickup_delay != INFINITE_PICKUP_DELAY {
            item.pickup_delay -= 1;
        }
        let tick_count = i32::from(item.age).wrapping_add(1);
        let old = current.pos;
        let mut pos = old;
        let mut v = world
            .get::<Velocity>(current.entity)
            .map_or(DVec3::ZERO, |v| v.0);
        v.y -= GRAVITY;
        let phase = tick_count.wrapping_add(current.entity.index_u32() as i32);
        let resting = on_ground(world, &blocks, current.dim, pos)
            && v.x * v.x + v.z * v.z <= REST_SPEED_SQ
            && phase % 4 != 0;
        if !resting {
            pos += v;
            if v.y < 0.0 {
                let column = BlockPos::from(old);
                let lowest = pos.y.floor() as i32;
                let highest = old.y.floor() as i32 - 1;
                if let Some(top) = (lowest..=highest).rev().find(|y| {
                    is_full_block(
                        world,
                        &blocks,
                        current.dim,
                        BlockPos::new(column.x, *y, column.z),
                    )
                }) {
                    pos.y = f64::from(top) + 1.0;
                    v.y = 0.0;
                }
            }
            let grounded = on_ground(world, &blocks, current.dim, pos);
            let friction = if grounded {
                let below = BlockPos::from(pos - DVec3::new(0.0, 0.5000001, 0.0));
                AIR_DRAG * f64::from(blocks.state(state_at(world, current.dim, below)).friction)
            } else {
                AIR_DRAG
            };
            v = DVec3::new(v.x * friction, v.y * AIR_DRAG, v.z * friction);
            if grounded && v.y < 0.0 {
                v.y *= -0.5;
            }
        }
        let moved = old.floor() != pos.floor();
        let rate = if moved {
            MERGE_RATE_MOVING
        } else {
            MERGE_RATE_RESTING
        };
        if item.age != INFINITE_LIFETIME {
            item.age += 1;
        }
        let mut entity = world.entity_mut(current.entity);
        entity.get_mut::<Transform>().unwrap().translation = pos;
        entity.insert(Velocity(v));
        *entity.get_mut::<DroppedItem>().unwrap() = item;
        if item.age >= LIFETIME {
            world.despawn(current.entity);
            continue;
        }
        if tick_count % rate == 0 {
            merge_with_neighbours(world, &items, &snapshot, current.entity, current.dim, pos);
        }
    }
}

fn is_mergeable(world: &World, entity: Entity) -> bool {
    let Some((item, stack)) = world
        .get::<DroppedItem>(entity)
        .zip(world.get::<ItemStack>(entity))
    else {
        return false;
    };
    item.pickup_delay != INFINITE_PICKUP_DELAY
        && item.age != INFINITE_LIFETIME
        && item.age < LIFETIME
        && stack.count < max_stack_size(world.entity(entity))
}

fn merge_with_neighbours(
    world: &mut World,
    items: &Items,
    snapshot: &[Snapshot],
    entity: Entity,
    dim: Entity,
    pos: DVec3,
) {
    if !is_mergeable(world, entity) {
        return;
    }
    let reach_xz = ITEM_WIDTH + 0.5;
    for other in snapshot {
        if other.entity == entity || other.dim != dim {
            continue;
        }
        let Some(other_pos) = world
            .get::<Transform>(other.entity)
            .map(|transform| transform.translation)
        else {
            continue;
        };
        let touching = (other_pos.x - pos.x).abs() < reach_xz
            && (other_pos.z - pos.z).abs() < reach_xz
            && other_pos.y < pos.y + ITEM_HEIGHT
            && other_pos.y + ITEM_HEIGHT > pos.y;
        if !touching || !is_mergeable(world, other.entity) {
            continue;
        }
        try_merge(world, items, entity, other.entity);
        if world.get_entity(entity).is_err() {
            return;
        }
    }
}

fn try_merge(world: &mut World, items: &Items, this: Entity, other: Entity) {
    let (this_count, other_count) = (count(world, this), count(world, other));
    let max = max_stack_size(world.entity(other));
    if usize::from(this_count) + usize::from(other_count) > usize::from(max)
        || !same_item_same_components(world, this, other, items)
    {
        return;
    }
    let (into, from) = if other_count < this_count {
        (this, other)
    } else {
        (other, this)
    };
    Transaction(vec![Op::MergeDropped { from, into }]).apply(world);
}

fn count(world: &World, stack: Entity) -> u8 {
    world.get::<ItemStack>(stack).map_or(0, |stack| stack.count)
}

fn state_at(world: &World, dim: Entity, pos: BlockPos) -> BlockStateId {
    world
        .get::<SectionIndex>(dim)
        .and_then(|index| index.get(pos))
        .and_then(|section| world.get::<ChunkBlocks>(section))
        .map_or(BlockStateId::default(), |blocks| {
            BlockStateId::from(blocks.get(LocalPos::from(pos)))
        })
}

fn is_full_block(world: &World, blocks: &Blocks, dim: Entity, pos: BlockPos) -> bool {
    blocks
        .state(state_at(world, dim, pos))
        .flags
        .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK)
}

fn on_ground(world: &World, blocks: &Blocks, dim: Entity, pos: DVec3) -> bool {
    pos.y.fract() == 0.0
        && is_full_block(
            world,
            blocks,
            dim,
            BlockPos::from(pos) - bevy_math::IVec3::new(0, 1, 0),
        )
}
