#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_inventory::value::spawn_stack;
use mcrs_minecraft_inventory::{Op, Slot, Transaction, TransactionError};
use mcrs_minecraft_item::{Items, SlotTable, StackRevision, load_item_definitions, stack_to_value};
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemStackValue, Template};

pub fn corpus() -> &'static (Blocks, Items) {
    static CORPUS: OnceLock<(Blocks, Items)> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        let (blocks, _) = load_block_definitions(&asset_server).expect("the block corpus loads");
        let items = load_item_definitions(&asset_server, &blocks).expect("the item corpus loads");
        (Blocks(Arc::new(blocks)), Items(Arc::new(items)))
    })
}

pub fn items() -> &'static Items {
    &corpus().1
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

pub fn template(path: &str, count: i32, components: ComponentPatch) -> Template {
    Template(value(path, count, components))
}

/// A stack in no holder.
pub fn spawn(world: &mut World, path: &str, count: i32) -> Entity {
    spawn_stack(world, &value(path, count, ComponentPatch::EMPTY), items()).unwrap()
}

pub fn holder(world: &mut World, cells: usize) -> Entity {
    world.spawn(SlotTable::fixed(cells)).id()
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
