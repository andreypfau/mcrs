use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::SlotChanged;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Rarity;

use crate::fixture::PLAIN;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::request_id;
use crate::fixture::spawn;

struct Case {
    name: &'static str,
    command: fn(&mut World, Entity, Entity),
    changed: &'static [u16],
}

fn at(holder: Entity, index: u16) -> SlotRef {
    SlotRef { holder, index }
}

const CASES: [Case; 6] = [
    Case {
        name: "put_at",
        command: |world, holder, _| {
            let stack = spawn(world, ItemStack::new(PLAIN, count(1)));
            world.commands().entity(stack).put_at(holder, 1);
        },
        changed: &[1],
    },
    Case {
        name: "consume",
        command: |world, _, stack| {
            world.commands().entity(stack).consume(1);
        },
        changed: &[0],
    },
    Case {
        name: "set",
        command: |world, _, stack| {
            world.commands().entity(stack).set(Rarity::Rare);
        },
        changed: &[0],
    },
    Case {
        name: "a merge by give",
        command: |world, holder, _| {
            world
                .commands()
                .entity(holder)
                .give(ItemStack::new(PLAIN, count(3)));
        },
        changed: &[0],
    },
    Case {
        name: "a request commit",
        command: |world, holder, _| {
            let request = Request::new(request_id(world)).split(at(holder, 0), at(holder, 2), 2);
            world.commands().entity(holder).request(request);
        },
        changed: &[0, 2],
    },
    Case {
        name: "load_inventory",
        command: |world, holder, _| {
            let values = vec![None, Some(ItemStack::new(PLAIN, count(1))), None];
            world.commands().entity(holder).load_inventory(values);
        },
        changed: &[0, 1],
    },
];

#[test]
fn each_slot_write_writes_slot_changed_for_each_changed_slot() {
    for case in CASES {
        let mut app = app();
        let world = app.world_mut();
        let holder = world.spawn(Slots::new(3)).id();
        let stack = spawn(world, ItemStack::new(PLAIN, count(5)));
        world.commands().entity(stack).put_at(holder, 0);
        world.flush();
        world.resource_mut::<Messages<SlotChanged>>().clear();

        (case.command)(world, holder, stack);
        world.flush();

        let mut changed: Vec<u16> = world
            .resource_mut::<Messages<SlotChanged>>()
            .drain()
            .map(|message| {
                assert_eq!(message.slot.holder, holder, "{}", case.name);
                message.slot.index
            })
            .collect();
        changed.sort_unstable();
        changed.dedup();
        assert_eq!(changed, case.changed, "{}", case.name);
    }
}
