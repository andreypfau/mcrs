use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_inventory_menu::MenuEntry;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_vanilla_inventory::PlayerInventories;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::gameplay_app;
use crate::fixture::holders;
use crate::fixture::patched_stone;
use crate::fixture::put;
use crate::fixture::restore;
use crate::fixture::stacks;

fn stack(kind: Kind, count: u8) -> ItemStack {
    ItemStack::new(kind, Count::new(count).expect("a count above 0"))
}

#[test]
fn clear_items_despawns_the_stacks_of_the_four_holders() {
    let mut app = app();
    let world = app.world_mut();
    let player = restore(world, &PersistedSnapshot::new());
    let holders = holders(world, player);
    put(world, player, 0, STONE, 5);
    put(world, player, 40, DIRT, 1);
    put(world, holders.cursor, 0, STONE, 2);
    put(world, holders.craft_grid, 3, DIRT, 4);
    put(world, holders.craft_result, 0, STONE, 1);
    let cursor_stack = world
        .get::<Slots>(holders.cursor)
        .and_then(|slots| slots.get(0))
        .expect("the cursor holds a stack");

    world.commands().entity(player).clear_items();
    world.flush();

    for holder in [
        player,
        holders.cursor,
        holders.craft_grid,
        holders.craft_result,
    ] {
        assert_eq!(
            stacks(world, holder),
            Vec::new(),
            "the holder {holder} holds nothing"
        );
    }
    assert!(
        world.get_entity(cursor_stack).is_err(),
        "the stack of the cursor despawns"
    );
}

#[test]
fn capture_gives_back_the_restored_items() {
    let mut app = app();
    let world = app.world_mut();
    let player = restore(world, &PersistedSnapshot::new());
    let mut items = PlayerItems {
        cursor: Some(stack(DIRT, 7)),
        selected: 5,
        ..PlayerItems::default()
    };
    items.slots[3] = Some(patched_stone());
    items.slots[40] = Some(stack(STONE, 64));
    items.craft_grid[2] = Some(stack(DIRT, 1));
    let other = world.spawn_empty().id();

    world.commands().entity(player).restore_items(items.clone());
    world.flush();
    let captured = world
        .run_system_once(move |mut inventories: PlayerInventories| {
            (inventories.capture(player), inventories.capture(other))
        })
        .expect("the capture runs");

    assert_eq!(captured.0, Some(items), "the restored values come back");
    assert_eq!(captured.1, None, "an entity without holders is no player");
}

#[test]
fn hand_swap_and_close_change_the_slots_without_the_packet_edge() {
    let mut app = gameplay_app();
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();
    let holders = holders(world, player);
    put(world, player, 0, STONE, 5);
    put(world, player, 40, DIRT, 1);
    put(world, holders.cursor, 0, STONE, 2);

    world
        .commands()
        .entity(player)
        .hand_action(HandAction::SwapHands);
    world.flush();
    assert_eq!(
        stacks(world, player),
        vec![(0, DIRT, 1), (40, STONE, 5)],
        "the selected slot 0 and the off hand exchange their stacks"
    );

    world.commands().entity(player).close_menu();
    world.flush();
    assert_eq!(
        stacks(world, player),
        vec![(0, DIRT, 1), (40, STONE, 7)],
        "the cursor stack joins the alike stack of the off hand, not the unlike selected slot"
    );
    assert_eq!(
        stacks(world, holders.cursor),
        Vec::new(),
        "the cursor is empty after the close"
    );
}

#[test]
fn closing_a_second_menu_moves_the_cursor_and_keeps_the_craft_grid() {
    let mut app = gameplay_app();
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();
    let holders = holders(world, player);
    put(world, holders.cursor, 0, STONE, 3);
    put(world, holders.craft_grid, 1, DIRT, 2);

    let chest = world.spawn(Slots::new(27)).id();
    let entries = (0..27).map(|index| MenuEntry { part: 1, index }).collect();
    let layout = MenuLayout::new(MenuType::Generic9x3, entries, vec![]).expect("a valid layout");
    let parts = MenuParts::new([player, chest]).expect("two parts");
    world
        .commands()
        .entity(player)
        .open_menu(layout, parts, MenuButtons::default());
    world.flush();

    world.commands().entity(player).close_menu();
    world.flush();
    assert_eq!(
        stacks(world, player),
        vec![(0, STONE, 3)],
        "the cursor stack moves into the player inventory"
    );
    assert_eq!(
        stacks(world, holders.craft_grid),
        vec![(1, DIRT, 2)],
        "the craft grid keeps its stack when another menu closes"
    );
}
