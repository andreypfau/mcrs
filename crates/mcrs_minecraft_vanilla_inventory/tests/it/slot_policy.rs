use std::sync::LazyLock;

use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::Reason;
use mcrs_minecraft_engine_item::Refusal;
use mcrs_minecraft_engine_item::Rejected;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_game_registries::registry_load::load_registries;
use mcrs_minecraft_game_registries::registry_load::test_additions;
use mcrs_minecraft_game_registries::registry_load::test_packs;
use mcrs_minecraft_inventory_player::OffHandRule;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_vanilla_inventory::CLOSE_REQUEST;
use mcrs_minecraft_vanilla_inventory::HAND_SWAP_REQUEST;
use mcrs_minecraft_vanilla_inventory::HandAction;
use mcrs_minecraft_vanilla_inventory::InventoryPersistencePlugin;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::capture;
use crate::fixture::dropped;
use crate::fixture::gameplay_app;
use crate::fixture::gameplay_app_with;
use crate::fixture::holders;
use crate::fixture::put;
use crate::fixture::restore;
use crate::fixture::stacks;

/// The test registries with the item tag `mcrs:offhand_allowed` holding stone alone.
static OFFHAND_STONE: LazyLock<Registries> = LazyLock::new(|| {
    let mut additions = test_additions();
    additions.files.insert(
        "offhand",
        "data/mcrs/tags/item/offhand_allowed.json",
        r#"{"values":["minecraft:stone"]}"#,
    );
    additions.embedded_packs.push("offhand".to_owned());
    let (registries, _) = load_registries(&test_packs(), additions).expect("the pack loads");
    registries
});

fn results(world: &World) -> Vec<RequestResult> {
    let results = world.resource::<Messages<RequestResult>>();
    results.iter_current_update_messages().copied().collect()
}

#[test]
fn the_off_hand_rule_decides_which_item_a_hand_swap_moves_into_the_off_hand() {
    struct Case {
        rule: OffHandRule,
        item: Kind,
        outcome: Result<(), Refusal>,
    }
    let refused = Err(Refusal::Rejected(Rejected {
        op: 0,
        reason: Reason::PlaceDenied,
    }));
    let cases = [
        Case {
            rule: OffHandRule::TaggedItems,
            item: DIRT,
            outcome: refused,
        },
        Case {
            rule: OffHandRule::TaggedItems,
            item: STONE,
            outcome: Ok(()),
        },
        Case {
            rule: OffHandRule::AnyItem,
            item: DIRT,
            outcome: Ok(()),
        },
    ];
    for case in cases {
        let plugin = VanillaInventoryPlugin {
            off_hand: case.rule,
        };
        let mut app = gameplay_app_with(OFFHAND_STONE.clone(), plugin);
        let world = app.world_mut();
        let player = world.spawn(Player).id();
        world.flush();
        put(world, player, 0, case.item, 3);

        world
            .commands()
            .entity(player)
            .hand_action(HandAction::SwapHands);
        world.flush();

        let label = format!("{:?} with item {:?}", case.rule, case.item);
        let result = RequestResult {
            holder: player,
            id: HAND_SWAP_REQUEST,
            outcome: case.outcome,
        };
        assert_eq!(results(world), [result], "{label}");
        let expected = match case.outcome {
            Ok(()) => (40, case.item, 3),
            Err(_) => (0, case.item, 3),
        };
        assert_eq!(stacks(world, player), [expected], "{label}");
    }
}

#[test]
fn a_close_returns_the_cursor_stack_past_an_observer_that_denies_each_request() {
    let mut app = gameplay_app();
    app.add_observer(|mut check: On<RequestCheck>| {
        check.deny(Denial {
            source: CLOSE_REQUEST,
            code: 1,
        });
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
    off_hand: OffHandRule,
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
    let plugin = VanillaInventoryPlugin {
        off_hand: case.off_hand,
    };
    let mut app = gameplay_app_with(OFFHAND_STONE.clone(), plugin);
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

/// INV-CLS-005: the insert of a close skips each slot whose place filter refuses the stack, fills
/// each slot up to its capacity (ITX-LIM-002), and the items that find no room become one item
/// entity.
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
            label: "an off hand that refuses the item",
            off_hand: OffHandRule::TaggedItems,
            before: vec![(40, DIRT, 10)],
            access: None,
            cursor: (DIRT, 20),
            after: vec![(0, DIRT, 20), (40, DIRT, 10)],
            dropped: vec![],
        },
        CloseCase {
            label: "a main slot that a plugin locks",
            off_hand: OffHandRule::default(),
            before: vec![(3, STONE, 10)],
            access: Some(locked(3)),
            cursor: (STONE, 20),
            after: vec![(0, STONE, 20), (3, STONE, 10)],
            dropped: vec![],
        },
        CloseCase {
            label: "a locked main slot among main slots of dirt",
            off_hand: OffHandRule::default(),
            before: dirt_around_slot_3,
            access: Some(locked(3)),
            cursor: (STONE, 20),
            after: all_dirt_after,
            dropped: vec![(STONE, 20)],
        },
        CloseCase {
            label: "an empty main slot with a slot limit below the stack",
            off_hand: OffHandRule::default(),
            before: vec![],
            access: Some(limit_16(0)),
            cursor: (STONE, 20),
            after: vec![(0, STONE, 16), (1, STONE, 4)],
            dropped: vec![],
        },
        CloseCase {
            label: "an alike main slot with a slot limit below the stack",
            off_hand: OffHandRule::default(),
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

/// INV-CLS-005: a close moves the stacks of a cursor and a craft grid that refuse each take.
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
