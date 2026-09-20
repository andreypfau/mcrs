#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::{Items, load_item_definitions};
use mcrs_minecraft_item::{DirtyStacks, SlotTable, mutate};
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
    world.init_resource::<DirtyStacks>();
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

pub fn spawn(world: &mut World, path: &str, count: i32) -> Entity {
    mutate::spawn_stack(world, &value(path, count, ComponentPatch::EMPTY), items()).unwrap()
}

pub fn holder(world: &mut World, cells: usize) -> Entity {
    world.spawn(SlotTable::fixed(cells)).id()
}

pub fn drain(world: &mut World) -> DirtyStacks {
    std::mem::take(&mut *world.resource_mut::<DirtyStacks>())
}
