use bevy::prelude::*;
use mcrs_minecraft_client::test_support::ServerAnswers;
use mcrs_minecraft_client::test_support::count_server_answers;
use mcrs_minecraft_client::test_support::creative_player;
use mcrs_minecraft_client::test_support::give;
use mcrs_minecraft_client::test_support::item_kind;
use mcrs_minecraft_client::test_support::joined;
use mcrs_minecraft_client::test_support::open;
use mcrs_minecraft_client::test_support::player_slot;
use mcrs_minecraft_client::test_support::put_local;
use mcrs_minecraft_client::test_support::round_trip;
use mcrs_minecraft_client::test_support::update_until;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_client::MenuClick;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_save::SaveLock;

use crate::local_world::client;
use crate::local_world::world_folder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Stone,
    Dirt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stack {
    item: Item,
    count: u8,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    entry: u16,
    stack: Option<Stack>,
}

struct Row {
    name: &'static str,
    given: &'static [Entry],
    local: &'static [Entry],
    clicks: &'static [Click],
    entries: &'static [Entry],
    cursor: Option<Stack>,
    answers: ServerAnswers,
}

const fn held(item: Item, count: u8) -> Option<Stack> {
    Some(Stack { item, count })
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

const fn click(kind: ClickKind, button: u8, clicked: Clicked) -> Click {
    Click {
        kind,
        button,
        clicked,
    }
}

const fn on(kind: ClickKind, button: u8, entry: u16) -> Click {
    click(kind, button, Clicked::Entry(entry))
}

const AGREED: ServerAnswers = ServerAnswers {
    slots: 0,
    cursors: 0,
    contents: 0,
    player_slots: 0,
};

const ENTRIES: std::ops::RangeInclusive<u16> = 9..=45;

const ROWS: &[Row] = &[
    Row {
        name: "pick up",
        given: &[at(9, Item::Stone, 10)],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 0, 9)],
        entries: &[empty(9)],
        cursor: held(Item::Stone, 10),
        answers: AGREED,
    },
    Row {
        name: "place",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 0, 10)],
        entries: &[at(10, Item::Stone, 10)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "split with a right click",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 1, 10)],
        entries: &[at(10, Item::Stone, 5)],
        cursor: held(Item::Stone, 5),
        answers: AGREED,
    },
    Row {
        name: "swap with a cursor stack",
        given: &[at(11, Item::Dirt, 3)],
        local: &[],
        clicks: &[on(ClickKind::PickUp, 0, 11)],
        entries: &[at(11, Item::Stone, 5)],
        cursor: held(Item::Dirt, 3),
        answers: AGREED,
    },
    Row {
        name: "drag of three slots",
        given: &[],
        local: &[],
        clicks: &[
            click(ClickKind::Drag, 0, Clicked::Outside),
            on(ClickKind::Drag, 1, 12),
            on(ClickKind::Drag, 1, 13),
            on(ClickKind::Drag, 1, 14),
            click(ClickKind::Drag, 2, Clicked::Outside),
        ],
        entries: &[
            at(12, Item::Dirt, 1),
            at(13, Item::Dirt, 1),
            at(14, Item::Dirt, 1),
        ],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "quick move",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::QuickMove, 0, 10)],
        entries: &[empty(10), at(36, Item::Stone, 5)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "number key",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Swap, 2, 11)],
        entries: &[empty(11), at(38, Item::Stone, 5)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "off-hand key",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Swap, 40, 38)],
        entries: &[empty(38), at(45, Item::Stone, 5)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "clone in creative",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Clone, 2, 45)],
        entries: &[at(45, Item::Stone, 5)],
        cursor: held(Item::Stone, 64),
        answers: AGREED,
    },
    Row {
        name: "outside the window drops the cursor",
        given: &[],
        local: &[],
        clicks: &[click(ClickKind::PickUp, 0, Clicked::Outside)],
        entries: &[],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "throw one",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Throw, 0, 36)],
        entries: &[at(36, Item::Stone, 4)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "throw the stack",
        given: &[],
        local: &[],
        clicks: &[on(ClickKind::Throw, 1, 36)],
        entries: &[empty(36)],
        cursor: None,
        answers: AGREED,
    },
    Row {
        name: "a wrong prediction is repaired slot by slot",
        given: &[at(20, Item::Stone, 8)],
        local: &[at(20, Item::Stone, 3)],
        clicks: &[on(ClickKind::PickUp, 1, 20)],
        entries: &[at(20, Item::Stone, 4)],
        cursor: held(Item::Stone, 4),
        answers: ServerAnswers {
            slots: 1,
            cursors: 1,
            ..AGREED
        },
    },
];

#[test]
fn each_click_leaves_the_client_and_the_server_with_the_same_stacks() {
    let folder = world_folder("menu-clicks");
    let mut app = client();
    count_server_answers(&mut app);
    open(&mut app, &folder);
    update_until(&mut app, "the join", joined);
    update_until(&mut app, "the inventory menu in creative mode", |app| {
        app.world_mut().run_schedule(FixedUpdate);
        app.world().resource::<ServerAnswers>().contents > 0
            && creative_player(app.world_mut()).is_some()
    });
    round_trip(&mut app);
    let viewer = creative_player(app.world_mut()).unwrap();

    let mut expected: Vec<Option<Stack>> = ENTRIES.map(|_| None).collect();
    for row in ROWS {
        for given in row.given {
            let value = value(app.world(), given.stack);
            give(app.world_mut(), viewer, given.entry, value);
            expected[usize::from(given.entry - ENTRIES.start())] = given.stack;
        }
        for local in row.local {
            let value = value(app.world(), local.stack);
            put_local(app.world_mut(), viewer, local.entry, value);
        }
        *app.world_mut().resource_mut::<ServerAnswers>() = ServerAnswers::default();
        for &click in row.clicks {
            app.world_mut().write_message(MenuClick { viewer, click });
            app.world_mut().run_schedule(FixedUpdate);
        }
        round_trip(&mut app);

        assert_eq!(
            *app.world().resource::<ServerAnswers>(),
            row.answers,
            "{}: the packets of the server in answer",
            row.name
        );
        for entry in row.entries {
            expected[usize::from(entry.entry - ENTRIES.start())] = entry.stack;
        }
        let entries: Vec<Option<Stack>> = ENTRIES
            .map(|entry| stack_at(app.world(), viewer, player_slot(entry)))
            .collect();
        assert_eq!(entries, expected, "{}: the entries 9 to 45", row.name);
        assert_eq!(
            cursor(app.world(), viewer),
            row.cursor,
            "{}: the cursor",
            row.name
        );
    }

    app.world_mut().write_message(AppExit::Success);
    app.update();
    let lock = SaveLock::take(&folder).expect("the server released the world");
    drop(lock);
    std::fs::remove_dir_all(&folder).ok();
}

fn kind(world: &World, item: Item) -> Kind {
    let name = match item {
        Item::Stone => "minecraft:stone",
        Item::Dirt => "minecraft:dirt",
    };
    item_kind(world, name)
}

fn value(world: &World, stack: Option<Stack>) -> Option<ItemStack> {
    let stack = stack?;
    Some(ItemStack::new(
        kind(world, stack.item),
        Count::new(stack.count).unwrap(),
    ))
}

fn stack_at(world: &World, holder: Entity, index: u16) -> Option<Stack> {
    let stack = world.entity(world.get::<Slots>(holder)?.get(usize::from(index))?);
    let found = *stack.get::<Kind>()?;
    let item = [Item::Stone, Item::Dirt]
        .into_iter()
        .find(|item| kind(world, *item) == found)
        .expect("the test gives only stone and dirt");
    Some(Stack {
        item,
        count: stack.get::<Count>()?.get(),
    })
}

fn cursor(world: &World, viewer: Entity) -> Option<Stack> {
    let holders = world.get::<PlayerHolders>(viewer)?;
    stack_at(world, holders.cursor, 0)
}
