use bevy_app::FixedUpdate;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::Schedules;
use bevy_ecs::schedule::SingleThreadedExecutor;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_test_alloc::allocations;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::apply;

const PLAYERS: u64 = 8;

#[test]
fn the_sync_of_unchanged_inventories_allocates_nothing() {
    let mut app = app();
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::default());
    }
    let world = app.world_mut();
    for session in 0..PLAYERS {
        let player = world.spawn((Player, Owner(PlayerSession(session)))).id();
        apply(world, player, &PersistedSnapshot::new());
        for index in 0..36 {
            let count = Count::new(index + 1).expect("a count above 0");
            let stack = if index % 2 == 0 {
                ItemStack::new(STONE, count)
            } else {
                ItemStack::new(DIRT, count).set(MaxStackSize(40))
            };
            world
                .commands()
                .spawn_stack(stack)
                .put_at(player, u16::from(index));
        }
        world.flush();
    }
    for _ in 0..3 {
        world.run_schedule(FixedUpdate);
    }
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .clear();

    let before = allocations();
    world.run_schedule(FixedUpdate);
    let allocated = allocations() - before;

    let sent = world.resource::<Messages<OutboundPlayerPacket>>().len();
    assert_eq!((allocated, sent), (0, 0));
}
