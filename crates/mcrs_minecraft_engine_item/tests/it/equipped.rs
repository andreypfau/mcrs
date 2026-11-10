use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Insert;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_engine_item::Equipped;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_item_stack::ItemStack;

use crate::fixture::PLAIN;
use crate::fixture::TABLE;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::spawn;

/// Holder `a` selects slot 0 and holds `stacks[0]` and `stacks[1]` in slots 0 and 1. Holder `b`
/// selects slot 1 and holds `stacks[2]` there.
struct Inventories {
    a: Entity,
    b: Entity,
    stacks: [Entity; 3],
}

fn inventories(app: &mut App) -> Inventories {
    let world = app.world_mut();
    let a = world.spawn((Slots::new(4), Selected(0))).id();
    let b = world.spawn((Slots::new(4), Selected(1))).id();
    let places = [(a, 0), (a, 1), (b, 1)];
    let stacks = places.map(|(holder, index)| {
        let stack = spawn(world, ItemStack::new(PLAIN, count(1)));
        world.commands().entity(stack).put_at(holder, index);
        stack
    });
    world.flush();
    tick(app);
    Inventories { a, b, stacks }
}

fn tick(app: &mut App) {
    app.world_mut().run_schedule(FixedUpdate);
}

fn equipped(world: &mut World) -> Vec<Entity> {
    let mut stacks: Vec<Entity> = (world.query_filtered::<Entity, With<Equipped>>())
        .iter(world)
        .collect();
    stacks.sort();
    stacks
}

struct Case {
    name: &'static str,
    act: fn(&mut World, &Inventories),
    equipped: fn(&Inventories) -> Vec<Entity>,
}

const CASES: [Case; 12] = [
    Case {
        name: "initial placement",
        act: |_, _| {},
        equipped: |at| vec![at.stacks[0], at.stacks[2]],
    },
    Case {
        name: "change of the selection",
        act: |world, at| {
            world.entity_mut(at.a).insert(Selected(1));
        },
        equipped: |at| vec![at.stacks[1], at.stacks[2]],
    },
    Case {
        name: "put_at into the selected slot",
        act: |world, at| {
            world.commands().entity(at.stacks[1]).put_at(at.a, 0);
        },
        equipped: |at| vec![at.stacks[1], at.stacks[2]],
    },
    Case {
        name: "put_at out of the selected slot",
        act: |world, at| {
            world.commands().entity(at.stacks[0]).put_at(at.a, 3);
        },
        equipped: |at| vec![at.stacks[2]],
    },
    Case {
        name: "put_at from the selected slot into the selected slot of another holder",
        act: |world, at| {
            world.entity_mut(at.b).insert(Selected(2));
            world.commands().entity(at.stacks[0]).put_at(at.b, 2);
        },
        equipped: |at| vec![at.stacks[0]],
    },
    Case {
        name: "swap_with",
        act: |world, at| {
            world
                .commands()
                .entity(at.stacks[0])
                .swap_with(at.stacks[1]);
        },
        equipped: |at| vec![at.stacks[1], at.stacks[2]],
    },
    Case {
        name: "request that swaps across holders",
        act: |world, at| {
            let left = SlotRef {
                holder: at.a,
                index: 1,
            };
            let right = SlotRef {
                holder: at.b,
                index: 1,
            };
            let request = Request::new(RequestId(1)).swap(left, right);
            world.commands().entity(at.a).request(request);
        },
        equipped: |at| vec![at.stacks[0], at.stacks[1]],
    },
    Case {
        name: "drop_at",
        act: |world, at| {
            world.commands().entity(at.stacks[2]).drop_at(DVec3::ZERO);
        },
        equipped: |at| vec![at.stacks[0]],
    },
    Case {
        name: "despawn of the equipped stack",
        act: |world, at| {
            world.despawn(at.stacks[0]);
        },
        equipped: |at| vec![at.stacks[2]],
    },
    Case {
        name: "selection out of range",
        act: |world, at| {
            world.entity_mut(at.a).insert(Selected(200));
        },
        equipped: |at| vec![at.stacks[2]],
    },
    Case {
        name: "removal of the selection",
        act: |world, at| {
            world.entity_mut(at.b).remove::<Selected>();
        },
        equipped: |at| vec![at.stacks[0]],
    },
    Case {
        name: "two holders change in one tick",
        act: |world, at| {
            world.entity_mut(at.a).insert(Selected(1));
            world.entity_mut(at.b).insert(Selected(0));
            world.commands().entity(at.stacks[0]).put_at(at.b, 0);
        },
        equipped: |at| vec![at.stacks[0], at.stacks[1]],
    },
];

#[test]
fn exactly_the_stack_in_the_selected_slot_of_each_holder_is_equipped() {
    for case in CASES {
        let mut app = app();
        let at = inventories(&mut app);
        let world = app.world_mut();
        (case.act)(world, &at);
        world.flush();
        tick(&mut app);
        let mut expected = (case.equipped)(&at);
        expected.sort();
        assert_eq!(equipped(app.world_mut()), expected, "{}", case.name);
    }
}

#[test]
fn a_removal_of_the_selection_in_frames_without_a_fixed_step_unequips() {
    let mut app = app();
    let at = inventories(&mut app);
    app.world_mut().entity_mut(at.b).remove::<Selected>();
    app.update();
    app.update();

    tick(&mut app);

    assert_eq!(equipped(app.world_mut()), [at.stacks[0]]);
}

#[derive(Resource, Default)]
struct Writes(usize);

#[test]
fn a_tick_without_a_change_writes_no_equipped() {
    let mut app = app();
    inventories(&mut app);
    app.init_resource::<Writes>()
        .add_observer(|_: On<Insert, Equipped>, mut writes: ResMut<Writes>| writes.0 += 1)
        .add_observer(|_: On<Remove, Equipped>, mut writes: ResMut<Writes>| writes.0 += 1);

    tick(&mut app);
    tick(&mut app);

    assert_eq!(app.world().resource::<Writes>().0, 0);
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct Custom;

#[test]
fn the_projection_runs_in_the_schedule_the_plugin_names() {
    let mut app = App::new();
    app.add_plugins(ItemPlugin::new(*TABLE).in_schedule(Custom));
    let world = app.world_mut();
    let holder = world.spawn((Slots::new(1), Selected(0))).id();
    let stack = spawn(world, ItemStack::new(PLAIN, count(1)));
    world.commands().entity(stack).put_at(holder, 0);
    world.flush();

    app.world_mut().run_schedule(Custom);
    assert_eq!(equipped(app.world_mut()), [stack]);
}
