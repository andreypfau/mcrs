use bevy::prelude::*;
use mcrs_minecraft_client::test_support::ServerAnswers;
use mcrs_minecraft_client::test_support::player_slot;
use mcrs_minecraft_client::test_support::received_creative_slots;
use mcrs_minecraft_client::test_support::record_creative_slots;
use mcrs_minecraft_client::test_support::round_trip;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_client::CreativeArea;
use mcrs_minecraft_inventory_client::CreativeClick;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::UndecodedValue;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::local_world::{self, ENTRIES, LocalWorld};
use crate::stacks::AGREED;
use crate::stacks::Item;
use crate::stacks::Stack;
use crate::stacks::cursor;
use crate::stacks::held;
use crate::stacks::item_of;
use crate::stacks::stack_at;
use crate::stacks::value;

#[derive(Clone, Copy, Debug)]
enum Area {
    Grid(Option<Item>),
    LockedGrid(Item),
    UnencodableGrid(Item),
    Hotbar,
    Inventory,
    Destroy,
}

#[derive(Clone, Copy, Debug)]
struct Action {
    area: Area,
    kind: ClickKind,
    button: u8,
    clicked: Clicked,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    entry: u16,
    stack: Option<Stack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sent {
    slot: i16,
    stack: Option<Stack>,
}

struct Row {
    name: &'static str,
    game_mode: GameMode,
    actions: &'static [Action],
    sent: &'static [Sent],
    entries: &'static [Entry],
    cursor: Option<Stack>,
}

const fn grid(item: Item, kind: ClickKind, button: u8) -> Action {
    Action {
        area: Area::Grid(Some(item)),
        kind,
        button,
        clicked: Clicked::Outside,
    }
}

const fn empty_grid(kind: ClickKind, button: u8) -> Action {
    Action {
        area: Area::Grid(None),
        kind,
        button,
        clicked: Clicked::Outside,
    }
}

const fn outside(button: u8) -> Action {
    Action {
        area: Area::Inventory,
        kind: ClickKind::PickUp,
        button,
        clicked: Clicked::Outside,
    }
}

const fn sent(slot: i16, item: Item, count: u8) -> Sent {
    Sent {
        slot,
        stack: held(item, count),
    }
}

const fn sent_empty(slot: i16) -> Sent {
    Sent { slot, stack: None }
}

const fn on(area: Area, kind: ClickKind, entry: u16) -> Action {
    Action {
        area,
        kind,
        button: 0,
        clicked: Clicked::Entry(entry),
    }
}

const fn at(entry: u16, item: Item, count: u8) -> Entry {
    Entry {
        entry,
        stack: held(item, count),
    }
}

const fn empty(entry: u16) -> Entry {
    Entry { entry, stack: None }
}

const ROWS: &[Row] = &[
    Row {
        name: "a full stack from the grid into the hotbar",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::Stone, ClickKind::QuickMove, 0),
            on(Area::Hotbar, ClickKind::PickUp, 36),
        ],
        sent: &[sent(36, Item::Stone, 64)],
        entries: &[at(36, Item::Stone, 64)],
        cursor: None,
    },
    Row {
        name: "take one from the grid",
        game_mode: GameMode::Creative,
        actions: &[grid(Item::Dirt, ClickKind::PickUp, 0)],
        sent: &[],
        entries: &[],
        cursor: held(Item::Dirt, 1),
    },
    Row {
        name: "a grid click on a stack with the slot lock has no result",
        game_mode: GameMode::Creative,
        actions: &[Action {
            area: Area::LockedGrid(Item::Stone),
            kind: ClickKind::PickUp,
            button: 0,
            clicked: Clicked::Outside,
        }],
        sent: &[],
        entries: &[],
        cursor: held(Item::Dirt, 1),
    },
    Row {
        name: "put it down in the inventory",
        game_mode: GameMode::Creative,
        actions: &[on(Area::Inventory, ClickKind::PickUp, 9)],
        sent: &[sent(9, Item::Dirt, 1)],
        entries: &[at(9, Item::Dirt, 1)],
        cursor: None,
    },
    Row {
        name: "clone from the grid",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::Dirt, ClickKind::Clone, 2),
            on(Area::Hotbar, ClickKind::PickUp, 37),
        ],
        sent: &[sent(37, Item::Dirt, 64)],
        entries: &[at(37, Item::Dirt, 64)],
        cursor: None,
    },
    Row {
        name: "a grid click with another stack deletes the cursor",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::Stone, ClickKind::PickUp, 0),
            grid(Item::Dirt, ClickKind::PickUp, 0),
        ],
        sent: &[],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "a quick move on the hotbar row deletes the stack",
        game_mode: GameMode::Creative,
        actions: &[on(Area::Hotbar, ClickKind::QuickMove, 36)],
        sent: &[sent_empty(36)],
        entries: &[empty(36)],
        cursor: None,
    },
    Row {
        name: "a quick move on the hotbar row over a non-hotbar entry changes nothing",
        game_mode: GameMode::Creative,
        actions: &[on(Area::Hotbar, ClickKind::QuickMove, 9)],
        sent: &[],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "survival refuses each creative action",
        game_mode: GameMode::Survival,
        actions: &[
            grid(Item::Stone, ClickKind::Clone, 2),
            on(Area::Hotbar, ClickKind::QuickMove, 37),
        ],
        sent: &[],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "a quick move on the destroy slot empties the inventory menu",
        game_mode: GameMode::Creative,
        actions: &[on(Area::Destroy, ClickKind::QuickMove, 0)],
        sent: &[sent_empty(9), sent_empty(37)],
        entries: &[empty(9), empty(37)],
        cursor: None,
    },
    Row {
        name: "a click outside drops the cursor",
        game_mode: GameMode::Creative,
        actions: &[grid(Item::Stone, ClickKind::QuickMove, 0), outside(0)],
        sent: &[sent(-1, Item::Stone, 64)],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "a right click on an empty grid slot deletes one item",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::Stone, ClickKind::QuickMove, 0),
            empty_grid(ClickKind::PickUp, 1),
        ],
        sent: &[],
        entries: &[],
        cursor: held(Item::Stone, 63),
    },
    Row {
        name: "the swap key and a left click on an empty grid slot",
        game_mode: GameMode::Creative,
        actions: &[
            empty_grid(ClickKind::Swap, 0),
            empty_grid(ClickKind::PickUp, 0),
        ],
        sent: &[],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "a click outside with each button",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::Stone, ClickKind::QuickMove, 0),
            outside(1),
            outside(0),
        ],
        sent: &[sent(-1, Item::Stone, 1), sent(-1, Item::Stone, 63)],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "a throw from a grid slot",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::Dirt, ClickKind::Throw, 0),
            grid(Item::Dirt, ClickKind::Throw, 1),
        ],
        sent: &[sent(-1, Item::Dirt, 1), sent(-1, Item::Dirt, 64)],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "the swap key over a grid stack",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::Stone, ClickKind::Swap, 2),
            grid(Item::Dirt, ClickKind::Swap, 40),
        ],
        sent: &[sent(38, Item::Stone, 64), sent(45, Item::Dirt, 64)],
        entries: &[at(38, Item::Stone, 64), at(45, Item::Dirt, 64)],
        cursor: None,
    },
    Row {
        name: "a throw from the hotbar row",
        game_mode: GameMode::Creative,
        actions: &[on(Area::Hotbar, ClickKind::Throw, 38)],
        sent: &[sent(-1, Item::Stone, 1), sent(38, Item::Stone, 63)],
        entries: &[at(38, Item::Stone, 63)],
        cursor: None,
    },
    Row {
        name: "a stack with no wire form does not leave the cursor for a slot",
        game_mode: GameMode::Creative,
        actions: &[
            Action {
                area: Area::UnencodableGrid(Item::Dirt),
                kind: ClickKind::PickUp,
                button: 0,
                clicked: Clicked::Outside,
            },
            on(Area::Inventory, ClickKind::PickUp, 10),
            on(Area::Hotbar, ClickKind::PickUp, 39),
            on(Area::Destroy, ClickKind::PickUp, 0),
        ],
        sent: &[],
        entries: &[],
        cursor: None,
    },
    Row {
        name: "an enchanted book from the grid into the hotbar",
        game_mode: GameMode::Creative,
        actions: &[
            grid(Item::EnchantedBook, ClickKind::QuickMove, 0),
            on(Area::Hotbar, ClickKind::PickUp, 39),
        ],
        sent: &[sent(39, Item::EnchantedBook, 1)],
        entries: &[at(39, Item::EnchantedBook, 1)],
        cursor: None,
    },
];

#[test]
fn each_creative_action_leaves_the_client_and_the_server_with_the_same_stacks() {
    let mut world = LocalWorld::join(
        "creative-actions",
        local_world::discard_thrown_items,
        record_creative_slots,
    );
    let viewer = world.viewer;

    let mut expected: Vec<Option<Stack>> = ENTRIES.map(|_| None).collect();
    for row in ROWS {
        world.switch_game_mode(row.game_mode);
        *world.app.world_mut().resource_mut::<ServerAnswers>() = ServerAnswers::default();
        received_creative_slots(world.app.world_mut());
        for action in row.actions {
            let input = MenuInput {
                viewer,
                action: MenuAction::Creative(creative_click(world.app.world(), action)),
            };
            world.app.world_mut().write_message(input);
        }
        world.app.update();
        round_trip(&mut world.app);

        assert_eq!(
            *world.app.world().resource::<ServerAnswers>(),
            AGREED,
            "{}: the packets of the server in answer",
            row.name
        );
        let received: Vec<Sent> = received_creative_slots(world.app.world_mut())
            .into_iter()
            .map(|received| Sent {
                slot: received.slot,
                stack: stack_of(world.app.world(), received.stack.as_ref()),
            })
            .collect();
        assert_eq!(
            received, row.sent,
            "{}: the creative slots that the server receives",
            row.name
        );
        for entry in row.entries {
            expected[usize::from(entry.entry - ENTRIES.start())] = entry.stack;
        }
        let entries: Vec<Option<Stack>> = ENTRIES
            .map(|entry| stack_at(world.app.world(), viewer, player_slot(entry)))
            .collect();
        assert_eq!(entries, expected, "{}: the entries 9 to 45", row.name);
        assert_eq!(
            cursor(world.app.world(), viewer),
            row.cursor,
            "{}: the cursor",
            row.name
        );
        if row.cursor.is_none() && row.game_mode == GameMode::Creative {
            assert_server_holds(&mut world, row);
        }
    }

    world.leave();
}

fn assert_server_holds(world: &mut LocalWorld, row: &Row) {
    let viewer = world.viewer;
    world.switch_game_mode(GameMode::Survival);
    *world.app.world_mut().resource_mut::<ServerAnswers>() = ServerAnswers::default();
    for entry in row.entries {
        let pick_up = Click {
            kind: ClickKind::PickUp,
            button: 0,
            clicked: Clicked::Entry(entry.entry),
        };
        for _ in 0..2 {
            world.app.world_mut().write_message(MenuInput {
                viewer,
                action: MenuAction::Click(pick_up),
            });
        }
    }
    world.app.update();
    round_trip(&mut world.app);
    assert_eq!(
        *world.app.world().resource::<ServerAnswers>(),
        AGREED,
        "{}: the server holds the stacks of the client",
        row.name
    );
    world.switch_game_mode(GameMode::Creative);
}

fn creative_click(world: &World, action: &Action) -> CreativeClick {
    let area = match action.area {
        Area::Grid(item) => CreativeArea::Grid(value(world, item.and_then(|item| held(item, 1)))),
        Area::LockedGrid(item) => CreativeArea::Grid(value(world, held(item, 1)).map(slot_locked)),
        Area::UnencodableGrid(item) => {
            CreativeArea::Grid(value(world, held(item, 1)).map(saved_value_only))
        }
        Area::Hotbar => CreativeArea::Hotbar,
        Area::Inventory => CreativeArea::Inventory,
        Area::Destroy => CreativeArea::Destroy,
    };
    CreativeClick {
        area,
        click: Click {
            kind: action.kind,
            button: action.button,
            clicked: action.clicked,
        },
    }
}

fn stack_of(world: &World, stack: Option<&ItemStack>) -> Option<Stack> {
    let stack = stack?;
    let read = Stack {
        item: item_of(world, stack.kind),
        count: stack.count.get(),
    };
    assert_eq!(value(world, Some(read)).as_ref(), Some(stack), "{read:?}");
    Some(read)
}

fn saved_value_only(stack: ItemStack) -> ItemStack {
    stack.with_entry(PatchEntry::Undecoded(UndecodedValue {
        name: ResourceLocationBuf::minecraft("writable_book_content").unwrap(),
        bytes: Box::new([]),
    }))
}

fn slot_locked(stack: ItemStack) -> ItemStack {
    stack.with_entry(PatchEntry::Undecoded(UndecodedValue {
        name: ResourceLocationBuf::minecraft("creative_slot_lock").unwrap(),
        bytes: Box::new([]),
    }))
}
