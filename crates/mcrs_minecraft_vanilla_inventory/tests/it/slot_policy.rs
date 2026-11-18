use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_vanilla_inventory::InventoryPersistencePlugin;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;
use mcrs_minecraft_vanilla_inventory::RequestPurpose;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryRequests;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::capture;
use crate::fixture::dropped;
use crate::fixture::gameplay_app;
use crate::fixture::holders;
use crate::fixture::put;
use crate::fixture::restore;
use crate::fixture::stacks;

fn results(world: &World) -> Vec<RequestResult> {
    let results = world.resource::<Messages<RequestResult>>();
    results.iter_current_update_messages().copied().collect()
}

#[test]
fn inv_slt_005_a_hand_swap_puts_any_item_in_the_off_hand() {
    let mut app = gameplay_app();
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();
    put(world, player, 0, DIRT, 3);

    world
        .commands()
        .entity(player)
        .hand_action(HandAction::SwapHands);
    world.flush();

    let result = RequestResult {
        holder: player,
        id: RequestId {
            source: world.resource::<VanillaInventoryRequests>().source,
            serial: RequestPurpose::HandSwap.serial(),
        },
        outcome: Ok(()),
    };
    assert_eq!(results(world), [result]);
    assert_eq!(stacks(world, player), [(40, DIRT, 3)]);
}

#[test]
fn a_close_returns_the_cursor_stack_past_an_observer_that_denies_each_request() {
    let mut app = gameplay_app();
    let source = app.allocate_request_source().unwrap();
    app.add_observer(move |mut check: On<RequestCheck>| {
        check.deny(Denial { source, code: 1 });
    });
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();
    let holders = holders(world, player);
    put(world, holders.cursor, 0, STONE, 2);

    world.commands().entity(player).close_menu();
    world.flush();

    assert_eq!(stacks(world, player), [(0, STONE, 2)]);
    assert_eq!(stacks(world, holders.cursor), []);
}

#[derive(Clone, Copy, Debug)]
enum ClosePath {
    Packet,
    Capture(CaptureCause),
}

fn close_by(world: &mut World, player: Entity, path: ClosePath) {
    match path {
        ClosePath::Packet => {
            world.commands().entity(player).close_menu();
            world.flush();
            world.run_schedule(FixedUpdate);
        }
        ClosePath::Capture(cause) => {
            capture(world, player, cause);
        }
    }
}

struct CloseCase {
    label: &'static str,
    before: Vec<(u16, Kind, u8)>,
    access: Option<PolicyEntry>,
    cursor: (Kind, u8),
    after: Vec<(usize, Kind, u8)>,
    dropped: Vec<(Kind, u8)>,
}

struct PolicyEntry {
    index: u16,
    access: SlotAccess,
}

fn assert_close(case: &CloseCase, path: ClosePath) {
    let mut app = gameplay_app();
    app.add_plugins(InventoryPersistencePlugin);
    let world = app.world_mut();
    let player = restore(world, &PersistedSnapshot::new());
    let holders = holders(world, player);
    for &(index, kind, count) in &case.before {
        put(world, player, index, kind, count);
    }
    if let Some(entry) = &case.access {
        world
            .get_mut::<SlotPolicy>(player)
            .expect("a player has a slot policy")
            .set(entry.index, entry.access);
    }
    put(world, holders.cursor, 0, case.cursor.0, case.cursor.1);

    close_by(world, player, path);

    let label = format!("{} by {path:?}", case.label);
    assert_eq!(stacks(world, player), case.after, "{label}");
    assert_eq!(stacks(world, holders.cursor), [], "{label}");
    let dropped: Vec<(Kind, u8)> = dropped(world)
        .into_iter()
        .map(|(kind, count, _)| (kind, count))
        .collect();
    assert_eq!(dropped, case.dropped, "{label}");
}

#[test]
fn inv_cls_005_a_close_skips_each_slot_whose_place_filter_refuses_the_stack() {
    let locked = |index| PolicyEntry {
        index,
        access: SlotAccess {
            limit: 99,
            take: Gate::Refuses,
            place: Gate::Refuses,
        },
    };
    let limit_16 = |index| PolicyEntry {
        index,
        access: SlotAccess {
            limit: 16,
            take: Gate::Permits,
            place: Gate::Permits,
        },
    };
    let dirt_around_slot_3: Vec<(u16, Kind, u8)> = (0..36)
        .map(|index| match index {
            3 => (index, STONE, 10),
            _ => (index, DIRT, 64),
        })
        .collect();
    let all_dirt_after: Vec<(usize, Kind, u8)> = dirt_around_slot_3
        .iter()
        .map(|&(index, kind, count)| (usize::from(index), kind, count))
        .collect();
    let cases = [
        CloseCase {
            label: "a main slot that a plugin locks",
            before: vec![(3, STONE, 10)],
            access: Some(locked(3)),
            cursor: (STONE, 20),
            after: vec![(0, STONE, 20), (3, STONE, 10)],
            dropped: vec![],
        },
        CloseCase {
            label: "a locked main slot among main slots of dirt",
            before: dirt_around_slot_3,
            access: Some(locked(3)),
            cursor: (STONE, 20),
            after: all_dirt_after,
            dropped: vec![(STONE, 20)],
        },
        CloseCase {
            label: "an empty main slot with a slot limit below the stack",
            before: vec![],
            access: Some(limit_16(0)),
            cursor: (STONE, 20),
            after: vec![(0, STONE, 16), (1, STONE, 4)],
            dropped: vec![],
        },
        CloseCase {
            label: "an alike main slot with a slot limit below the stack",
            before: vec![(0, STONE, 10)],
            access: Some(limit_16(0)),
            cursor: (STONE, 20),
            after: vec![(0, STONE, 16), (1, STONE, 14)],
            dropped: vec![],
        },
    ];
    let paths = [
        ClosePath::Packet,
        ClosePath::Capture(CaptureCause::Leave),
        ClosePath::Capture(CaptureCause::DimensionChange),
    ];
    for case in &cases {
        for path in paths {
            assert_close(case, path);
        }
    }
}

#[test]
fn inv_cls_005_a_close_reads_no_take_filter() {
    let mut app = gameplay_app();
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();
    let holders = holders(world, player);
    put(world, holders.cursor, 0, STONE, 2);
    put(world, holders.craft_grid, 1, DIRT, 3);
    for holder in [holders.cursor, holders.craft_grid] {
        world.entity_mut(holder).insert(SlotPolicy::locked());
    }

    world.commands().entity(player).close_menu();
    world.flush();

    assert_eq!(stacks(world, player), [(0, STONE, 2), (1, DIRT, 3)]);
    assert_eq!(stacks(world, holders.cursor), []);
    assert_eq!(stacks(world, holders.craft_grid), []);
}
