use bevy::prelude::*;
use mcrs_minecraft_client::test_support::ServerAnswers;
use mcrs_minecraft_client::test_support::connection;
use mcrs_minecraft_client::test_support::player_slot;
use mcrs_minecraft_client::test_support::round_trip;
use mcrs_minecraft_client::test_support::update_until;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuilders;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;
use mcrs_minecraft_inventory_client::RemoteMenu;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::local_world::LocalWorld;
use crate::stacks::AGREED;
use crate::stacks::Entry;
use crate::stacks::Item;
use crate::stacks::Stack;
use crate::stacks::at;
use crate::stacks::cursor;
use crate::stacks::empty;
use crate::stacks::held;
use crate::stacks::holding;
use crate::stacks::item_name;
use crate::stacks::on;
use crate::stacks::stack_at;

const OPEN_CHEST: &str = "chest";

const CLOSE_MENU: &str = "closemenu";

const CHEST_SIZE: u16 = 27;

const MENU_ENTRIES: u16 = 63;

const CHEST: &[Entry] = &[
    at(0, Item::Stone, 10),
    at(1, Item::Dirt, 4),
    at(2, Item::Stone, 7),
];

const GIVEN_ENTRY: u16 = 36;

const GIVEN: Stack = Stack {
    item: Item::Dirt,
    count: 3,
};

const GIVEN_MENU_ENTRY: u16 = 54;

struct Row {
    name: &'static str,
    click: Click,
    entries: &'static [Entry],
    cursor: Option<Stack>,
}

const ROWS: &[Row] = &[
    Row {
        name: "pick up from the container",
        click: on(ClickKind::PickUp, 0, 0),
        entries: &[empty(0)],
        cursor: held(Item::Stone, 10),
    },
    Row {
        name: "place into the player part",
        click: on(ClickKind::PickUp, 0, 28),
        entries: &[at(28, Item::Stone, 10)],
        cursor: None,
    },
    Row {
        name: "quick move from the container merges into the hotbar",
        click: on(ClickKind::QuickMove, 0, 1),
        entries: &[empty(1), at(54, Item::Dirt, 7)],
        cursor: None,
    },
    Row {
        name: "quick move from the player part merges into the container",
        click: on(ClickKind::QuickMove, 0, 28),
        entries: &[empty(28), at(2, Item::Stone, 17)],
        cursor: None,
    },
    Row {
        name: "number key swaps a container entry with the first hotbar slot",
        click: on(ClickKind::Swap, 0, 2),
        entries: &[at(2, Item::Dirt, 7), at(54, Item::Stone, 17)],
        cursor: None,
    },
    Row {
        name: "pick up the stack that the close puts back",
        click: on(ClickKind::PickUp, 0, 2),
        entries: &[empty(2)],
        cursor: held(Item::Dirt, 7),
    },
];

const AFTER_CLOSE: &[Entry] = &[at(36, Item::Stone, 17), at(37, Item::Dirt, 7)];

#[test]
fn a_server_container_menu_is_held_clicked_and_closed_by_the_client() {
    let mut local_world = LocalWorld::join("container-menus", open_chest_on_command, |_| {});
    let viewer = local_world.viewer;
    local_world.give(&[holding(GIVEN_ENTRY, Some(GIVEN))]);
    let app = &mut local_world.app;
    round_trip(app);

    let menu = open_chest(app, viewer, 1);
    let world = app.world();
    assert_eq!(
        world.get::<MenuLayout>(menu).map(MenuLayout::len),
        Some(MENU_ENTRIES),
        "a generic_9x3 menu has 63 entries"
    );
    assert_eq!(
        world.get::<MenuTitle>(menu),
        Some(&MenuTitle(Text::text("Chest"))),
        "the title of open_screen"
    );
    let mut expected: Vec<Option<Stack>> = (0..MENU_ENTRIES).map(|_| None).collect();
    for entry in CHEST {
        expected[usize::from(entry.entry)] = entry.stack;
    }
    expected[usize::from(GIVEN_MENU_ENTRY)] = Some(GIVEN);
    assert_eq!(
        menu_stacks(world, menu),
        expected,
        "the container and the player part at the open"
    );
    assert_eq!(cursor(world, viewer), None, "the cursor at the open");

    for row in ROWS {
        *app.world_mut().resource_mut::<ServerAnswers>() = ServerAnswers::default();
        app.world_mut().write_message(MenuInput {
            viewer,
            action: MenuAction::Click(row.click),
        });
        app.update();
        round_trip(app);

        assert_eq!(
            *app.world().resource::<ServerAnswers>(),
            AGREED,
            "{}: the packets of the server in answer",
            row.name
        );
        for entry in row.entries {
            expected[usize::from(entry.entry)] = entry.stack;
        }
        assert_eq!(
            menu_stacks(app.world(), menu),
            expected,
            "{}: the entries of the menu",
            row.name
        );
        assert_eq!(
            cursor(app.world(), viewer),
            row.cursor,
            "{}: the cursor",
            row.name
        );
    }

    close_with_a_cursor_stack(app, viewer);
    replace_the_open_menu(app, viewer);
    local_world.leave();
}

fn close_with_a_cursor_stack(app: &mut App, viewer: Entity) {
    app.world_mut().write_message(MenuInput {
        viewer,
        action: MenuAction::Close,
    });
    app.update();
    round_trip(app);
    let world = app.world();
    let menus: Vec<Option<i32>> = world
        .get::<Menus>(viewer)
        .unwrap()
        .iter()
        .map(|menu| world.get::<RemoteMenu>(menu).map(RemoteMenu::container_id))
        .collect();
    assert_eq!(menus, [Some(0)], "only the inventory menu after the close");
    let inventory: Vec<Option<Stack>> = (9..=45)
        .map(|entry| {
            AFTER_CLOSE
                .iter()
                .find(|after| after.entry == entry)
                .and_then(|after| after.stack)
        })
        .collect();
    let held_after_close: Vec<Option<Stack>> = (9..=45)
        .map(|entry| stack_at(world, viewer, player_slot(entry)))
        .collect();
    assert_eq!(
        held_after_close, inventory,
        "the insert rule puts the cursor stack into the first empty main slot"
    );
    assert_eq!(cursor(world, viewer), None, "the cursor after the close");
}

fn replace_the_open_menu(app: &mut App, viewer: Entity) {
    let first = open_chest(app, viewer, 2);
    let first_holder = container_holder(app.world(), first);
    let second = open_chest(app, viewer, 3);
    let world = app.world();
    assert!(
        world.get_entity(first).is_err(),
        "the replaced menu is gone"
    );
    assert!(
        world.get_entity(first_holder).is_err(),
        "the container holder of the replaced menu is gone"
    );
    assert_eq!(
        world.get::<Menus>(viewer).unwrap().iter().count(),
        2,
        "the inventory menu and the new menu"
    );
    let chest: Vec<Option<Stack>> = menu_stacks(world, second)
        .into_iter()
        .take(CHEST.len())
        .collect();
    let sent: Vec<Option<Stack>> = CHEST.iter().map(|entry| entry.stack).collect();
    assert_eq!(chest, sent, "the container of the new menu");
}

pub(crate) fn open_chest(app: &mut App, viewer: Entity, container_id: i32) -> Entity {
    connection(app.world_mut()).write_packet(&ServerboundChatCommand {
        command: Bounded(OPEN_CHEST),
    });
    update_until(app, |app| {
        open_container_id(app.world(), viewer) == Some(container_id)
    });
    round_trip(app);
    let menu = app
        .world()
        .get::<Menus>(viewer)
        .and_then(Menus::open)
        .unwrap();
    assert_eq!(
        app.world()
            .get::<RemoteMenu>(menu)
            .map(RemoteMenu::container_id),
        Some(container_id),
        "the container id that the server counts from the join"
    );
    menu
}

fn open_container_id(world: &World, viewer: Entity) -> Option<i32> {
    let menu = world.get::<Menus>(viewer)?.open()?;
    Some(world.get::<RemoteMenu>(menu)?.container_id())
}

fn container_holder(world: &World, menu: Entity) -> Entity {
    world
        .get::<MenuParts>(menu)
        .and_then(|parts| parts.holder(MenuLayout::GENERIC_CONTAINER))
        .unwrap()
}

pub(crate) fn menu_stacks(world: &World, menu: Entity) -> Vec<Option<Stack>> {
    let layout = world.get::<MenuLayout>(menu).unwrap();
    let parts = world.get::<MenuParts>(menu).unwrap();
    (0..layout.len())
        .map(|index| {
            let slot = parts.slot(layout.entry(index).unwrap()).unwrap();
            stack_at(world, slot.holder, slot.index)
        })
        .collect()
}

pub(crate) fn open_chest_on_command(server: &mut App) {
    server
        .world_mut()
        .get_resource_or_init::<DimensionBuilders>()
        .add(add_open_chest);
}

fn add_open_chest(_: &World, _: &DimensionBuild<'_>, dimension: &mut SubApp) {
    dimension.add_observer(open_chest_for_player);
}

fn open_chest_for_player(
    received: On<Received<ServerboundChatCommand<'static>>>,
    items: Res<ItemTable>,
    mut commands: Commands,
) {
    let player = received.entity;
    match received.packet().map(|packet| packet.command.0) {
        Some(OPEN_CHEST) => {}
        Some(CLOSE_MENU) => {
            commands.entity(player).close_menu();
            return;
        }
        _ => return,
    }
    let chest = commands.spawn(Slots::new(usize::from(CHEST_SIZE))).id();
    for entry in CHEST {
        let stack = entry.stack.unwrap();
        let location = ResourceLocation::read(item_name(stack.item)).unwrap();
        let kind = Kind(items.id(NameTable::Item, location).unwrap());
        commands
            .spawn_stack(ItemStack::new(kind, Count::new(stack.count).unwrap()))
            .put_at(chest, entry.entry);
    }
    commands.entity(player).open_titled_menu(
        MenuLayout::of(MenuType::Generic9x3).unwrap(),
        MenuParts::new([chest, player]).unwrap(),
        MenuButtons::default(),
        MenuTitle(Text::text("Chest")),
    );
}

pub(crate) fn close_on_server(app: &mut App) {
    connection(app.world_mut()).write_packet(&ServerboundChatCommand {
        command: Bounded(CLOSE_MENU),
    });
}
