use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::mouse::MouseButtonInput;
use bevy::input::{ButtonState, InputPlugin};
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client::test_support;
use mcrs_minecraft_client_gui::creative_screen_plugin::CreativeScreenPlugin;
use mcrs_minecraft_client_gui::creative_screen_state::CreativeScreenState;
use mcrs_minecraft_client_gui::creative_tab_content::CreativeTabContent;
use mcrs_minecraft_client_gui::inventory_clicks::InventoryClicksPlugin;
use mcrs_minecraft_client_gui::scene::GuiConfig;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_container_click::{Click, ClickKind, Clicked};
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_client::{CreativeArea, CreativeClick, MenuAction, MenuInput};
use mcrs_minecraft_item_stack::{Kind, write_wire};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_geometry::{CREATIVE_TABS, CreativeGeometry, CreativeTabIndex};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::{Decode, Encode};
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::local_world::LocalWorld;
use crate::stacks::{self, AGREED, Entry, Item, Stack};

const GUI_SCALE: u32 = 2;
const GUI_WIDTH: i32 = 640;
const GUI_HEIGHT: i32 = 360;
const ITEMS_TAB: usize = 0;
const NATURAL_BLOCKS_TAB: usize = 2;
const SEARCH_TAB: usize = 6;
const INVENTORY_TAB: usize = 13;
const DIRT_SLOT: u16 = 4;
const STONE_SLOT: u16 = 28;
const DESTROY_SLOT_ENTRY: u16 = 46;
const LEFT: MouseButton = MouseButton::Left;
const MIDDLE: MouseButton = MouseButton::Middle;
const SHIFT: KeyCode = KeyCode::ShiftLeft;

#[derive(Clone, Copy, Debug)]
enum Pointer {
    Tab(usize),
    Grid(u16),
    Hotbar(u16),
    Destroy,
    Outside,
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Move(Pointer),
    Press(MouseButton),
    Release(MouseButton),
    Key(KeyCode),
    Lift(KeyCode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Area {
    Grid(Option<Stack>),
    Hotbar,
    Inventory,
    Destroy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sent {
    area: Area,
    click: Click,
}

#[derive(Resource, Default)]
struct SentActions(Vec<MenuAction>);

struct Row {
    name: &'static str,
    tab: usize,
    given: &'static [Entry],
    given_cursor: Option<Stack>,
    server_cursor: Option<Stack>,
    frames: &'static [&'static [Event]],
    sent: &'static [Sent],
    selected: usize,
    screen: Screen,
    entries: &'static [Entry],
    cursor: Option<Stack>,
}

const fn row(name: &'static str, tab: usize) -> Row {
    Row {
        name,
        tab,
        given: &[],
        given_cursor: None,
        server_cursor: None,
        frames: &[],
        sent: &[],
        selected: tab,
        screen: Screen::Menu,
        entries: &[],
        cursor: None,
    }
}

const fn sent(area: Area, kind: ClickKind, button: u8, clicked: Clicked) -> Sent {
    Sent {
        area,
        click: stacks::click(kind, button, clicked),
    }
}

const fn over(pointer: Pointer) -> Event {
    Event::Move(pointer)
}

const ROWS: &[Row] = &[
    Row {
        frames: &[
            &[over(Pointer::Tab(NATURAL_BLOCKS_TAB)), Event::Press(LEFT)],
            &[Event::Release(LEFT)],
        ],
        selected: NATURAL_BLOCKS_TAB,
        ..row("a release on a tab selects it", ITEMS_TAB)
    },
    Row {
        frames: &[
            &[over(Pointer::Grid(DIRT_SLOT)), Event::Press(MIDDLE)],
            &[Event::Release(MIDDLE)],
        ],
        sent: &[sent(
            Area::Grid(stacks::held(Item::Dirt, 1)),
            ClickKind::Clone,
            2,
            Clicked::Entry(DIRT_SLOT),
        )],
        cursor: stacks::held(Item::Dirt, 64),
        ..row(
            "the middle button takes a full stack from the grid",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        frames: &[
            &[over(Pointer::Grid(STONE_SLOT)), Event::Press(LEFT)],
            &[Event::Release(LEFT)],
        ],
        sent: &[sent(
            Area::Grid(stacks::held(Item::Stone, 1)),
            ClickKind::PickUp,
            0,
            Clicked::Entry(STONE_SLOT),
        )],
        cursor: stacks::held(Item::Stone, 1),
        ..row(
            "a left press takes one item from the grid",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 64),
        frames: &[&[
            over(Pointer::Hotbar(0)),
            Event::Press(LEFT),
            Event::Release(LEFT),
        ]],
        sent: &[sent(Area::Hotbar, ClickKind::PickUp, 0, Clicked::Entry(36))],
        entries: &[stacks::at(36, Item::Stone, 64)],
        ..row(
            "a release over the hotbar row puts the cursor stack down",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        given: &[stacks::at(37, Item::Dirt, 5)],
        frames: &[
            &[over(Pointer::Hotbar(1)), Event::Press(MIDDLE)],
            &[Event::Release(MIDDLE)],
        ],
        sent: &[sent(Area::Hotbar, ClickKind::Clone, 2, Clicked::Entry(37))],
        cursor: stacks::held(Item::Dirt, 64),
        ..row(
            "the middle button clones a hotbar stack",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        given: &[
            stacks::at(9, Item::Stone, 10),
            stacks::at(36, Item::Dirt, 3),
        ],
        frames: &[
            &[
                Event::Key(SHIFT),
                over(Pointer::Destroy),
                Event::Press(LEFT),
            ],
            &[Event::Release(LEFT)],
        ],
        sent: &[sent(
            Area::Destroy,
            ClickKind::QuickMove,
            0,
            Clicked::Entry(DESTROY_SLOT_ENTRY),
        )],
        entries: &[stacks::empty(9), stacks::empty(36)],
        ..row(
            "a shift click on the destroy slot empties the inventory",
            INVENTORY_TAB,
        )
    },
    Row {
        given_cursor: stacks::held(Item::Dirt, 3),
        frames: &[&[
            over(Pointer::Outside),
            Event::Press(LEFT),
            Event::Release(LEFT),
        ]],
        sent: &[sent(
            Area::Inventory,
            ClickKind::PickUp,
            0,
            Clicked::Outside,
        )],
        ..row(
            "a release outside the screen drops the cursor stack",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        given: &[stacks::at(38, Item::Stone, 5)],
        frames: &[
            &[
                Event::Key(SHIFT),
                over(Pointer::Hotbar(2)),
                Event::Press(LEFT),
            ],
            &[Event::Release(LEFT)],
        ],
        sent: &[sent(
            Area::Hotbar,
            ClickKind::QuickMove,
            0,
            Clicked::Entry(38),
        )],
        entries: &[stacks::empty(38)],
        ..row(
            "a shift click on a hotbar stack deletes it",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        frames: &[&[over(Pointer::Grid(DIRT_SLOT)), Event::Key(KeyCode::Digit3)]],
        sent: &[sent(
            Area::Grid(stacks::held(Item::Dirt, 1)),
            ClickKind::Swap,
            2,
            Clicked::Entry(DIRT_SLOT),
        )],
        entries: &[stacks::at(38, Item::Dirt, 64)],
        ..row(
            "a number key over a grid stack puts a full stack into the hotbar",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        frames: &[&[Event::Key(KeyCode::KeyE)]],
        ..row(
            "the inventory key does not close the search tab",
            SEARCH_TAB,
        )
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[over(Pointer::Hotbar(0)), Event::Press(LEFT)],
            &[over(Pointer::Hotbar(1))],
            &[over(Pointer::Hotbar(2))],
            &[Event::Release(LEFT)],
        ],
        sent: &[
            sent(Area::Inventory, ClickKind::Drag, 0, Clicked::Outside),
            sent(Area::Inventory, ClickKind::Drag, 1, Clicked::Entry(37)),
            sent(Area::Inventory, ClickKind::Drag, 1, Clicked::Entry(38)),
            sent(Area::Inventory, ClickKind::Drag, 2, Clicked::Outside),
        ],
        entries: &[
            stacks::at(37, Item::Stone, 5),
            stacks::at(38, Item::Stone, 5),
        ],
        ..row(
            "a left drag over the hotbar row splits the cursor stack with creative clicks",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        frames: &[
            &[over(Pointer::Grid(STONE_SLOT)), Event::Press(LEFT)],
            &[Event::Release(LEFT)],
            &[Event::Press(LEFT)],
            &[Event::Release(LEFT)],
        ],
        sent: &[
            sent(
                Area::Grid(stacks::held(Item::Stone, 1)),
                ClickKind::PickUp,
                0,
                Clicked::Entry(STONE_SLOT),
            ),
            sent(
                Area::Grid(stacks::held(Item::Stone, 1)),
                ClickKind::PickUp,
                0,
                Clicked::Entry(STONE_SLOT),
            ),
        ],
        cursor: stacks::held(Item::Stone, 2),
        ..row(
            "a double click on a grid slot takes one item at the press and one at the release",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 5),
        frames: &[&[Event::Key(KeyCode::Escape)]],
        screen: Screen::None,
        ..row(
            "the close of the creative screen deletes the cursor stack",
            NATURAL_BLOCKS_TAB,
        )
    },
    Row {
        given_cursor: stacks::held(Item::Dirt, 3),
        server_cursor: stacks::held(Item::Stone, 5),
        cursor: stacks::held(Item::Dirt, 3),
        ..row(
            "the creative screen ignores the cursor stack of the server",
            NATURAL_BLOCKS_TAB,
        )
    },
];

fn gui_point(pointer: Pointer) -> IVec2 {
    let window = CreativeGeometry::window(GUI_WIDTH, GUI_HEIGHT);
    let corner = match pointer {
        Pointer::Tab(tab) => {
            let rect = CREATIVE_TABS.get(tab).unwrap().click_rect();
            return IVec2::new(window.x, window.y)
                + IVec2::new(rect.x + rect.width / 2, rect.y + rect.height / 2);
        }
        Pointer::Outside => return IVec2::new(window.x - 10, window.y + 20),
        Pointer::Grid(slot) => CreativeGeometry::grid_slot(slot).unwrap(),
        Pointer::Hotbar(column) => CreativeGeometry::hotbar_slot(column).unwrap(),
        Pointer::Destroy => CreativeGeometry::DESTROY_SLOT,
    };
    IVec2::new(window.x + corner.x, window.y + corner.y) + 8
}

fn record_actions(mut inputs: MessageReader<MenuInput>, mut sent: ResMut<SentActions>) {
    sent.0
        .extend(inputs.read().map(|input| input.action.clone()));
}

fn add_input(app: &mut App) {
    if !app.is_plugin_added::<InputPlugin>() {
        app.add_plugins(InputPlugin);
    }
    app.add_plugins((InventoryClicksPlugin, CreativeScreenPlugin))
        .insert_resource(GuiConfig {
            scale: GUI_SCALE,
            cursor: None,
            frozen_ticks: None,
            advanced_tooltips: false,
        })
        .init_resource::<SentActions>()
        .add_systems(Last, record_actions);
    app.world_mut()
        .spawn((Window::default(), CursorOptions::default(), PrimaryWindow));
}

fn play(app: &mut App, frame: &[Event]) {
    let world = app.world_mut();
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .unwrap();
    for &event in frame {
        let key = |key_code, state| KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window,
        };
        let mouse = |button, state| MouseButtonInput {
            button,
            state,
            window,
        };
        match event {
            Event::Move(pointer) => {
                let physical = gui_point(pointer) * GUI_SCALE.cast_signed() + 1;
                let mut window = world.get_mut::<Window>(window).unwrap();
                window.set_physical_cursor_position(Some(physical.as_dvec2()));
            }
            Event::Press(button) => {
                world.write_message(mouse(button, ButtonState::Pressed));
            }
            Event::Release(button) => {
                world.write_message(mouse(button, ButtonState::Released));
            }
            Event::Key(code) => {
                world.write_message(key(code, ButtonState::Pressed));
            }
            Event::Lift(code) => {
                world.write_message(key(code, ButtonState::Released));
            }
        }
    }
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
}

fn pick_up(world: &mut LocalWorld, entry: u16) {
    let viewer = world.viewer;
    world.app.world_mut().write_message(MenuInput {
        viewer,
        action: MenuAction::Click(stacks::click(ClickKind::PickUp, 0, Clicked::Entry(entry))),
    });
    world.app.update();
}

fn creative_click(world: &mut LocalWorld, area: CreativeArea, count: u8) {
    let viewer = world.viewer;
    let click = CreativeClick {
        area,
        click: stacks::click(ClickKind::PickUp, 0, Clicked::Outside),
    };
    for _ in 0..count {
        world.app.world_mut().write_message(MenuInput {
            viewer,
            action: MenuAction::Creative(click.clone()),
        });
    }
    world.app.update();
}

fn send_server_cursor(world: &mut LocalWorld, stack: Option<Stack>) {
    let app_world = world.app.world();
    let value = stacks::value(app_world, stack);
    let names = RegistryNames::new(
        app_world.resource::<CurrentRegistries>(),
        app_world.resource::<ItemTable>(),
    );
    let mut bytes = Vec::new();
    write_wire(value.as_ref(), &names, &mut bytes).unwrap();
    let contents = ProtoStack::decode(&mut bytes.as_slice()).unwrap();
    let mut body = Vec::new();
    ClientboundSetCursorItem { contents }
        .encode(&mut body)
        .unwrap();
    let viewer = world.viewer;
    world
        .app
        .world_mut()
        .trigger(Received::<ClientboundSetCursorItem>::new(
            viewer,
            body.into(),
        ));
}

fn set_up(world: &mut LocalWorld, row: &Row) {
    world.app.insert_resource(Screen::Menu);
    creative_click(world, CreativeArea::Destroy, 1);
    if let Some(stack) = row.given_cursor {
        let shown = stacks::value(world.app.world(), stacks::held(stack.item, 1));
        creative_click(world, CreativeArea::Grid(shown), stack.count);
    }
    let cleared: Vec<Entry> = (9..=45).map(stacks::empty).collect();
    world.give(&cleared);
    world.give(row.given);
    test_support::round_trip(&mut world.app);
    world
        .app
        .world_mut()
        .resource_scope(|world, mut screen: Mut<CreativeScreenState>| {
            let tab = CreativeTabIndex::new(row.tab).expect("a creative tab");
            screen.select(tab, world.resource::<CreativeTabContent>());
        });
    if row.server_cursor.is_some() {
        send_server_cursor(world, row.server_cursor);
    }
}

fn stack_of(world: &World, kind: Kind, count: u8) -> Stack {
    Stack {
        item: stacks::item_of(world, kind),
        count,
    }
}

fn sent_of(world: &World, action: &MenuAction) -> Option<Sent> {
    let (area, click) = match action {
        MenuAction::Creative(CreativeClick { area, click }) => (area, click),
        MenuAction::Close => return None,
        MenuAction::Click(_) | _ => {
            panic!("the creative screen sends only creative clicks, not {action:?}")
        }
    };
    let area = match area {
        CreativeArea::Grid(shown) => Area::Grid(
            shown
                .as_ref()
                .map(|shown| stack_of(world, shown.kind, shown.count.get())),
        ),
        CreativeArea::Hotbar => Area::Hotbar,
        CreativeArea::Inventory => Area::Inventory,
        CreativeArea::Destroy => Area::Destroy,
    };
    Some(Sent {
        area,
        click: *click,
    })
}

fn assert_server_holds(world: &mut LocalWorld, row: &Row) {
    world.switch_game_mode(GameMode::Survival);
    world.clear_answers();
    for entry in row.entries {
        for _ in 0..2 {
            pick_up(world, entry.entry);
        }
    }
    world.assert_after_round_trip(row.name, Some(AGREED), &[], None);
    world.switch_game_mode(GameMode::Creative);
}

#[test]
fn each_gesture_on_the_creative_screen_gives_its_creative_click_and_the_same_stacks_on_both_sides()
{
    let mut world = LocalWorld::join("creative-screen-clicks", |_| {}, add_input);
    world.app.insert_resource(Screen::Menu);
    world.switch_game_mode(GameMode::Creative);
    for row in ROWS {
        set_up(&mut world, row);
        world.clear_answers();
        world
            .app
            .world_mut()
            .resource_mut::<SentActions>()
            .0
            .clear();
        for frame in row.frames {
            play(&mut world.app, frame);
        }
        let held: Vec<Event> = (world.app.world().resource::<ButtonInput<KeyCode>>())
            .get_pressed()
            .map(|&key| Event::Lift(key))
            .collect();
        play(&mut world.app, &held);
        let app_world = world.app.world();
        let sent: Vec<Sent> = (app_world.resource::<SentActions>().0.iter())
            .filter_map(|action| sent_of(app_world, action))
            .collect();
        assert_eq!(sent, row.sent, "{}: the inputs", row.name);
        assert_eq!(
            app_world
                .resource::<CreativeScreenState>()
                .selected()
                .index(),
            row.selected,
            "{}: the selected tab",
            row.name
        );
        assert_eq!(
            *app_world.resource::<Screen>(),
            row.screen,
            "{}: the screen",
            row.name
        );
        world.assert_after_round_trip(row.name, Some(AGREED), row.entries, row.cursor);
        if row.cursor.is_none() {
            assert_server_holds(&mut world, row);
        }
    }
    world.leave();
}
