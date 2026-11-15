use std::sync::LazyLock;

use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::Reason;
use mcrs_minecraft_engine_item::Refusal;
use mcrs_minecraft_engine_item::Rejected;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_item::RequestResult;
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
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::gameplay_app;
use crate::fixture::gameplay_app_with;
use crate::fixture::holders;
use crate::fixture::put;
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
