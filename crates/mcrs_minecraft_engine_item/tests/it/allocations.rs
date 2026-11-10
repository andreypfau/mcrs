use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::Schedules;
use bevy_ecs::schedule::SingleThreadedExecutor;
use bevy_ecs::system::SystemState;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::StackRows;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_test_alloc::allocations;

use crate::fixture::LORED;
use crate::fixture::PLAIN;
use crate::fixture::SWORD;
use crate::fixture::TABLE;
use crate::fixture::app;
use crate::fixture::count;

const WARM_UP: u8 = 3;
const MEASURED: u8 = 32;

fn spawn_item(world: &mut World) -> Entity {
    let stack = world.commands().spawn_item(LORED).id();
    world.flush();
    stack
}

#[test]
fn an_unpatched_spawn_and_a_merge_check_allocate_nothing() {
    let mut world = World::new();
    world.insert_resource(*TABLE);
    let mut state = SystemState::<StackRows>::new(&mut world);
    let sword = world.commands().spawn_item(SWORD).id();
    world.flush();
    for _ in 0..8 {
        let stack = spawn_item(&mut world);
        world.despawn(stack);
    }

    let before = allocations();
    let left = spawn_item(&mut world);
    let spawned = allocations() - before;
    let right = spawn_item(&mut world);
    assert!(state.get(&world).unwrap().mergeable(left, right));
    assert!(!state.get(&world).unwrap().mergeable(left, sword));

    let before = allocations();
    let mergeable = state.get(&world).unwrap().mergeable(left, right);
    let merge_checked = allocations() - before;

    assert!(mergeable);
    assert_eq!((spawned, merge_checked), (0, 0));
}

fn holder_with(world: &mut World, items: u8) -> Entity {
    let holder = world.spawn(Slots::new(1)).id();
    let stack = world
        .commands()
        .spawn_stack(ItemStack::new(PLAIN, count(items)))
        .id();
    world.commands().entity(stack).put_at(holder, 0);
    world.flush();
    holder
}

#[test]
fn a_hopper_move_of_an_unpatched_stack_allocates_nothing() {
    let mut app = app();
    // The multi-threaded executor of Bevy allocates on each schedule run, with or without
    // systems; the single-threaded one leaves only the allocations of this crate.
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::default());
    }
    let source = holder_with(app.world_mut(), 64);
    let target = holder_with(app.world_mut(), 1);
    let from = SlotRef {
        holder: source,
        index: 0,
    };
    let to = SlotRef {
        holder: target,
        index: 0,
    };
    let hop = |app: &mut App| {
        let world = app.world_mut();
        let request = Request::new(RequestId(1)).split(from, to, 1);
        world.commands().entity(source).request(request);
        world.flush();
        app.update();
    };
    for _ in 0..WARM_UP {
        hop(&mut app);
    }

    let before = allocations();
    for _ in 0..MEASURED {
        hop(&mut app);
    }
    let moved = allocations() - before;

    let items = |holder: Entity| {
        let world = app.world();
        let stack = world.get::<Slots>(holder).unwrap().get(0).unwrap();
        world.get::<Count>(stack).unwrap().get()
    };
    let hops = WARM_UP + MEASURED;
    assert_eq!((items(source), items(target)), (64 - hops, 1 + hops));
    assert_eq!(moved, 0);
}
