#![allow(dead_code)]

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_inventory::value::spawn_stack;
use mcrs_minecraft_inventory::{Op, Slot, Transaction, TransactionError};
use mcrs_minecraft_item::{Items, SlotTable, StackRevision, stack_to_value, test_corpus};
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemComponentKind, ItemStackValue};

pub fn items() -> &'static Items {
    &test_corpus().1
}

pub fn world() -> World {
    let mut world = World::new();
    world.insert_resource(items().clone());
    world
}

pub fn value(path: &str, count: i32, components: ComponentPatch) -> ItemStackValue {
    ItemStackValue {
        item: ResourceKey::from_location(ResourceLocation::minecraft(path)),
        count: Bounded(count),
        components,
    }
}

/// A stack in no holder.
pub fn spawn(world: &mut World, path: &str, count: i32) -> Entity {
    spawn_stack(world, &value(path, count, ComponentPatch::EMPTY), items()).unwrap()
}

pub fn holder(world: &mut World, slots: usize) -> Entity {
    world.spawn(SlotTable::fixed(slots)).id()
}

pub fn remove(world: &mut World, stack: Entity, kind: ItemComponentKind) {
    apply(world, vec![Op::Remove { stack, kind }]).unwrap();
}

pub fn apply(world: &mut World, ops: Vec<Op>) -> Result<(), TransactionError> {
    Transaction(ops).try_apply(world)
}

pub fn place(
    world: &mut World,
    stack: Entity,
    holder: Entity,
    index: u16,
) -> Result<(), TransactionError> {
    apply(
        world,
        vec![Op::Place {
            stack,
            to: Slot::new(holder, index),
        }],
    )
}

pub fn set_count(world: &mut World, stack: Entity, count: u8) {
    let op = match count {
        0 => Op::Despawn { stack },
        count => {
            let mut value = stack_to_value(world, stack, items());
            value.count = Bounded(i32::from(count));
            Op::Apply { stack, value }
        }
    };
    apply(world, vec![op]).unwrap();
}

pub fn revision(world: &World, stack: Entity) -> u32 {
    world.get::<StackRevision>(stack).unwrap().0
}
