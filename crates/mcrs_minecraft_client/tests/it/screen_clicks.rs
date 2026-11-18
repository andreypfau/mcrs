use std::time::Duration;

use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::mouse::MouseButtonInput;
use bevy::input::{ButtonState, InputPlugin};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use mcrs_minecraft_client::hand_keys_plugin::HandKeysPlugin;
use mcrs_minecraft_client::test_support::{self, ServerAnswers};
use mcrs_minecraft_client_gui::inventory_clicks::InventoryClicksPlugin;
use mcrs_minecraft_client_gui::scene::GuiConfig;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_container_click::{Click, ClickKind, Clicked};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_client::{HandInput, MenuAction, MenuInput};
use mcrs_minecraft_menu_geometry::MenuGeometry;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_protocol::{Encode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerClose;

use crate::container_menus;
use crate::local_world::{self, LocalWorld};
use crate::stacks::{self, AGREED, Entry, Item, Stack};

const GUI_SCALE: u32 = 2;
const SCRATCH_ENTRY: u16 = 9;
const FRAME: Duration = Duration::from_millis(300);
const WINDOW_CORNER: IVec2 = IVec2::new((640 - 176) / 2, (360 - 166) / 2);

#[derive(Clone, Copy, Debug)]
enum Pointer {
    Entry(u16),
    ChestEntry(u16),
    EmptyArea,
    Outside,
    RecipeBookButton,
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Move(Pointer),
    Press(MouseButton),
    Release(MouseButton),
    Key(KeyCode),
    Repeat(KeyCode),
    Lift(KeyCode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sent {
    Click(Click),
    Hand(HandAction),
    Close,
}

#[derive(Resource, Default)]
struct SentInputs(Vec<Sent>);

struct Row {
    name: &'static str,
    game_mode: GameMode,
    screen: Screen,
    grab: CursorGrabMode,
    given: &'static [Entry],
    given_cursor: Option<Stack>,
    frames: &'static [&'static [Event]],
    sent: &'static [Sent],
    entries: &'static [Entry],
    cursor: Option<Stack>,
    answers: Option<ServerAnswers>,
}

const fn on(kind: ClickKind, button: u8, entry: u16) -> Sent {
    Sent::Click(stacks::click(kind, button, Clicked::Entry(entry)))
}

const fn outside(kind: ClickKind, button: u8) -> Sent {
    Sent::Click(stacks::click(kind, button, Clicked::Outside))
}

const fn over(entry: u16) -> Event {
    Event::Move(Pointer::Entry(entry))
}

const LEFT: MouseButton = MouseButton::Left;
const RIGHT: MouseButton = MouseButton::Right;
const MIDDLE: MouseButton = MouseButton::Middle;
const SHIFT: KeyCode = KeyCode::ShiftLeft;
const CONTROL: KeyCode = KeyCode::ControlLeft;

const fn row(name: &'static str) -> Row {
    Row {
        name,
        game_mode: GameMode::Creative,
        screen: Screen::Menu,
        grab: CursorGrabMode::Locked,
        given: &[],
        given_cursor: None,
        frames: &[],
        sent: &[],
        entries: &[],
        cursor: None,
        answers: Some(AGREED),
    }
}

const ROWS: &[Row] = &[
    Row {
        given: &[stacks::at(9, Item::Stone, 10)],
        frames: &[&[over(9), Event::Press(LEFT)], &[Event::Release(LEFT)]],
        sent: &[on(ClickKind::PickUp, 0, 9)],
        entries: &[stacks::empty(9)],
        cursor: stacks::held(Item::Stone, 10),
        ..row("a left press picks up and its release gives nothing")
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[&[over(10), Event::Press(LEFT), Event::Release(LEFT)]],
        sent: &[on(ClickKind::PickUp, 0, 10)],
        entries: &[stacks::at(10, Item::Stone, 10)],
        ..row("a release with a cursor stack places it")
    },
    Row {
        given: &[stacks::at(10, Item::Stone, 10)],
        frames: &[&[over(10), Event::Press(RIGHT)], &[Event::Release(RIGHT)]],
        sent: &[on(ClickKind::PickUp, 1, 10)],
        entries: &[stacks::at(10, Item::Stone, 5)],
        cursor: stacks::held(Item::Stone, 5),
        ..row("a right press splits")
    },
    Row {
        given: &[stacks::at(10, Item::Stone, 5)],
        given_cursor: stacks::held(Item::Stone, 5),
        frames: &[&[
            over(10),
            Event::Press(LEFT),
            Event::Press(RIGHT),
            Event::Release(RIGHT),
            Event::Release(LEFT),
        ]],
        entries: &[stacks::at(10, Item::Stone, 5)],
        cursor: stacks::held(Item::Stone, 5),
        ..row("a release of another button closes the drag and the next release gives no click")
    },
    Row {
        given: &[stacks::at(11, Item::Dirt, 3)],
        given_cursor: stacks::held(Item::Stone, 5),
        frames: &[&[over(11), Event::Press(LEFT), Event::Release(LEFT)]],
        sent: &[on(ClickKind::PickUp, 0, 11)],
        entries: &[stacks::at(11, Item::Stone, 5)],
        cursor: stacks::held(Item::Dirt, 3),
        ..row("a release swaps with the cursor stack")
    },
    Row {
        given_cursor: stacks::held(Item::Dirt, 3),
        frames: &[&[
            Event::Move(Pointer::Outside),
            Event::Press(LEFT),
            Event::Release(LEFT),
        ]],
        sent: &[outside(ClickKind::PickUp, 0)],
        ..row("a release outside the window drops the cursor stack")
    },
    Row {
        frames: &[&[
            Event::Move(Pointer::Outside),
            Event::Press(LEFT),
            Event::Release(LEFT),
        ]],
        sent: &[outside(ClickKind::Throw, 0)],
        ..row("a press outside the window with the empty cursor throws")
    },
    Row {
        frames: &[
            &[Event::Move(Pointer::EmptyArea), Event::Press(LEFT)],
            &[Event::Release(LEFT)],
        ],
        ..row("a press in the empty area gives no click")
    },
    Row {
        given: &[stacks::at(10, Item::Stone, 5)],
        frames: &[
            &[Event::Key(SHIFT), over(10), Event::Press(LEFT)],
            &[Event::Release(LEFT)],
        ],
        sent: &[on(ClickKind::QuickMove, 0, 10)],
        entries: &[stacks::empty(10), stacks::at(36, Item::Stone, 5)],
        ..row("a press with Shift moves the stack to the hotbar")
    },
    Row {
        given: &[stacks::at(9, Item::Stone, 10)],
        frames: &[
            &[
                Event::Key(SHIFT),
                over(9),
                Event::Press(LEFT),
                Event::Key(KeyCode::Digit1),
            ],
            &[over(20)],
        ],
        sent: &[on(ClickKind::QuickMove, 0, 9), on(ClickKind::Swap, 0, 9)],
        entries: &[stacks::at(9, Item::Stone, 10), stacks::empty(36)],
        ..row("a key kept for the next frame swaps the entry under the pointer of its own frame")
    },
    Row {
        given: &[stacks::at(11, Item::Stone, 5)],
        frames: &[&[over(11), Event::Key(KeyCode::Digit3)]],
        sent: &[on(ClickKind::Swap, 2, 11)],
        entries: &[stacks::empty(11), stacks::at(38, Item::Stone, 5)],
        ..row("a number key swaps with the hotbar")
    },
    Row {
        given: &[stacks::at(38, Item::Stone, 5)],
        frames: &[&[over(38), Event::Key(KeyCode::KeyF)]],
        sent: &[on(ClickKind::Swap, 40, 38)],
        entries: &[stacks::empty(38), stacks::at(45, Item::Stone, 5)],
        ..row("the off-hand key swaps with the off hand")
    },
    Row {
        given: &[stacks::at(36, Item::Stone, 5)],
        frames: &[&[
            over(36),
            Event::Key(KeyCode::KeyQ),
            Event::Repeat(KeyCode::KeyQ),
        ]],
        sent: &[on(ClickKind::Throw, 0, 36), on(ClickKind::Throw, 0, 36)],
        entries: &[stacks::at(36, Item::Stone, 3)],
        ..row("the drop key and its repeat throw one item each")
    },
    Row {
        given: &[stacks::at(36, Item::Stone, 3)],
        frames: &[&[Event::Key(CONTROL), over(36), Event::Key(KeyCode::KeyQ)]],
        sent: &[on(ClickKind::Throw, 1, 36)],
        entries: &[stacks::empty(36)],
        ..row("Control and the drop key throw the stack")
    },
    Row {
        given: &[stacks::at(37, Item::Dirt, 1)],
        frames: &[&[
            over(37),
            Event::Key(KeyCode::KeyQ),
            Event::Repeat(KeyCode::KeyQ),
        ]],
        sent: &[on(ClickKind::Throw, 0, 37)],
        entries: &[stacks::empty(37)],
        ..row("the repeat of the drop key reads the stack that the first throw left")
    },
    Row {
        given: &[stacks::at(45, Item::Stone, 5)],
        frames: &[&[over(45), Event::Press(MIDDLE)], &[Event::Release(MIDDLE)]],
        sent: &[on(ClickKind::Clone, 2, 45)],
        cursor: stacks::held(Item::Stone, 64),
        ..row("the middle button clones in creative")
    },
    Row {
        given: &[stacks::at(9, Item::Stone, 10)],
        given_cursor: stacks::held(Item::Stone, 64),
        frames: &[
            &[
                Event::Move(Pointer::RecipeBookButton),
                Event::Press(LEFT),
                Event::Release(LEFT),
            ],
            &[over(20), Event::Press(LEFT), Event::Release(LEFT)],
        ],
        sent: &[on(ClickKind::PickUp, 0, 20)],
        entries: &[stacks::at(20, Item::Stone, 64)],
        ..row("a press on the recipe book button and its release give no click")
    },
    Row {
        game_mode: GameMode::Spectator,
        given: &[stacks::at(9, Item::Stone, 10)],
        frames: &[&[over(9), Event::Press(LEFT)], &[Event::Release(LEFT)]],
        sent: &[on(ClickKind::PickUp, 0, 9)],
        answers: Some(ServerAnswers {
            contents: 1,
            ..AGREED
        }),
        ..row("a spectator sends a press and keeps the stacks")
    },
    Row {
        game_mode: GameMode::Spectator,
        given: &[stacks::at(9, Item::Stone, 10)],
        frames: &[&[over(9), Event::Key(KeyCode::Digit1)]],
        sent: &[on(ClickKind::Swap, 0, 9)],
        answers: Some(ServerAnswers {
            contents: 1,
            ..AGREED
        }),
        ..row("a spectator sends a number key and keeps the stacks")
    },
    Row {
        given: &[
            stacks::at(9, Item::Stone, 10),
            stacks::at(37, Item::Dirt, 5),
        ],
        frames: &[
            &[
                over(9),
                Event::Key(KeyCode::Digit1),
                Event::Key(KeyCode::Digit2),
                Event::Key(KeyCode::KeyE),
            ],
            &[],
        ],
        sent: &[
            on(ClickKind::Swap, 0, 9),
            on(ClickKind::Swap, 1, 9),
            Sent::Close,
        ],
        entries: &[
            stacks::at(9, Item::Dirt, 5),
            stacks::at(36, Item::Stone, 10),
            stacks::empty(37),
        ],
        ..row("the clicks before the inventory key are sent before the close")
    },
    Row {
        frames: &[&[Event::Key(KeyCode::Escape)]],
        sent: &[Sent::Close],
        ..row("Escape closes the inventory screen")
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[over(9), Event::Press(LEFT)],
            &[over(10)],
            &[over(11)],
            &[over(12)],
            &[Event::Release(LEFT)],
        ],
        sent: &[
            outside(ClickKind::Drag, 0),
            on(ClickKind::Drag, 1, 10),
            on(ClickKind::Drag, 1, 11),
            on(ClickKind::Drag, 1, 12),
            outside(ClickKind::Drag, 2),
        ],
        entries: &[
            stacks::at(10, Item::Stone, 3),
            stacks::at(11, Item::Stone, 3),
            stacks::at(12, Item::Stone, 3),
        ],
        cursor: stacks::held(Item::Stone, 1),
        ..row("a left drag over three entries splits the cursor stack between them")
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[over(9), Event::Press(RIGHT)],
            &[over(10)],
            &[over(11)],
            &[Event::Release(RIGHT)],
        ],
        sent: &[
            outside(ClickKind::Drag, 4),
            on(ClickKind::Drag, 5, 10),
            on(ClickKind::Drag, 5, 11),
            outside(ClickKind::Drag, 6),
        ],
        entries: &[
            stacks::at(10, Item::Stone, 1),
            stacks::at(11, Item::Stone, 1),
        ],
        cursor: stacks::held(Item::Stone, 8),
        ..row("a right drag puts one item in each entry")
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[over(9), Event::Press(RIGHT)],
            &[over(10)],
            &[over(11)],
            &[Event::Move(Pointer::RecipeBookButton), Event::Press(LEFT)],
            &[Event::Release(RIGHT)],
        ],
        sent: &[
            outside(ClickKind::Drag, 4),
            on(ClickKind::Drag, 5, 10),
            on(ClickKind::Drag, 5, 11),
            outside(ClickKind::Drag, 6),
        ],
        entries: &[
            stacks::at(10, Item::Stone, 1),
            stacks::at(11, Item::Stone, 1),
        ],
        cursor: stacks::held(Item::Stone, 8),
        ..row("a left press on the recipe book button keeps an open drag")
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[over(9), Event::Press(MIDDLE)],
            &[over(10)],
            &[over(11)],
            &[Event::Release(MIDDLE)],
        ],
        sent: &[
            outside(ClickKind::Drag, 8),
            on(ClickKind::Drag, 9, 10),
            on(ClickKind::Drag, 9, 11),
            outside(ClickKind::Drag, 10),
        ],
        entries: &[
            stacks::at(10, Item::Stone, 64),
            stacks::at(11, Item::Stone, 64),
        ],
        ..row("a middle drag in creative fills each entry and empties the cursor")
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[over(9), Event::Press(LEFT)],
            &[over(10)],
            &[over(11)],
            &[Event::Press(RIGHT), Event::Release(RIGHT)],
            &[Event::Release(LEFT)],
        ],
        cursor: stacks::held(Item::Stone, 10),
        ..row("a release of another button cancels a drag over two entries")
    },
    Row {
        given: &[stacks::at(9, Item::Stone, 10)],
        frames: &[
            &[Event::Move(Pointer::EmptyArea), Event::Press(RIGHT)],
            &[over(9), Event::Press(LEFT)],
            &[Event::Release(LEFT)],
            &[over(10), Event::Release(RIGHT)],
        ],
        sent: &[on(ClickKind::PickUp, 0, 9), on(ClickKind::PickUp, 1, 10)],
        entries: &[stacks::empty(9), stacks::at(10, Item::Stone, 1)],
        cursor: stacks::held(Item::Stone, 9),
        ..row("only the first release after a press that sent a click is skipped")
    },
    Row {
        given: &[stacks::at(9, Item::Stone, 10)],
        frames: &[
            &[
                over(9),
                Event::Press(LEFT),
                Event::Release(LEFT),
                Event::Press(LEFT),
            ],
            &[Event::Press(RIGHT)],
            &[over(10)],
            &[over(11)],
            &[Event::Move(Pointer::Outside), Event::Release(LEFT)],
        ],
        sent: &[on(ClickKind::PickUp, 0, 9), outside(ClickKind::PickUp, 0)],
        entries: &[stacks::empty(9), stacks::empty(10), stacks::empty(11)],
        ..row("a drag opened by a double click adds no entry, also after a later press")
    },
    Row {
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[Event::Key(SHIFT), over(9), Event::Press(LEFT)],
            &[over(10)],
            &[over(11)],
            &[Event::Release(LEFT)],
        ],
        sent: &[
            outside(ClickKind::Drag, 0),
            on(ClickKind::Drag, 1, 10),
            on(ClickKind::Drag, 1, 11),
            outside(ClickKind::Drag, 2),
        ],
        entries: &[
            stacks::at(10, Item::Stone, 5),
            stacks::at(11, Item::Stone, 5),
        ],
        ..row("Shift has no effect on a drag over two entries")
    },
    Row {
        given: &[stacks::at(10, Item::Stone, 5)],
        given_cursor: stacks::held(Item::Dirt, 3),
        frames: &[
            &[Event::Key(SHIFT), over(10), Event::Press(LEFT)],
            &[Event::Release(LEFT)],
        ],
        sent: &[on(ClickKind::QuickMove, 0, 10)],
        entries: &[stacks::empty(10), stacks::at(36, Item::Stone, 5)],
        cursor: stacks::held(Item::Dirt, 3),
        ..row("a Shift drag that joins no entry moves the stack under the pointer")
    },
    Row {
        given: &[
            stacks::at(9, Item::Stone, 10),
            stacks::at(20, Item::Stone, 5),
            stacks::at(30, Item::Stone, 3),
        ],
        frames: &[
            &[
                over(9),
                Event::Press(LEFT),
                Event::Release(LEFT),
                Event::Press(LEFT),
                Event::Release(LEFT),
            ],
            &[],
        ],
        sent: &[on(ClickKind::PickUp, 0, 9), on(ClickKind::PickUpAll, 0, 9)],
        entries: &[stacks::empty(9), stacks::empty(20), stacks::empty(30)],
        cursor: stacks::held(Item::Stone, 18),
        ..row("a double click gathers the stacks of the cursor item")
    },
    Row {
        given: &[
            stacks::at(9, Item::Stone, 10),
            stacks::at(20, Item::Stone, 5),
            stacks::at(30, Item::Stone, 3),
        ],
        frames: &[
            &[Event::Move(Pointer::EmptyArea), Event::Press(RIGHT)],
            &[
                over(9),
                Event::Press(LEFT),
                Event::Release(LEFT),
                Event::Press(LEFT),
            ],
            &[Event::Release(RIGHT)],
            &[Event::Release(LEFT)],
        ],
        sent: &[on(ClickKind::PickUp, 0, 9), on(ClickKind::PickUpAll, 0, 9)],
        entries: &[stacks::empty(9), stacks::empty(20), stacks::empty(30)],
        cursor: stacks::held(Item::Stone, 18),
        ..row("a double click gathers after a release of another button cancels its drag")
    },
    Row {
        given: &[stacks::at(20, Item::Stone, 5)],
        given_cursor: stacks::held(Item::Stone, 10),
        frames: &[
            &[Event::Move(Pointer::EmptyArea), Event::Press(RIGHT)],
            &[
                Event::Move(Pointer::RecipeBookButton),
                Event::Press(LEFT),
                Event::Release(LEFT),
                Event::Press(LEFT),
            ],
            &[over(9), Event::Release(LEFT)],
            &[over(10), Event::Release(RIGHT)],
        ],
        sent: &[on(ClickKind::PickUpAll, 0, 9), on(ClickKind::PickUp, 1, 10)],
        entries: &[stacks::empty(20), stacks::at(10, Item::Stone, 1)],
        cursor: stacks::held(Item::Stone, 14),
        ..row("the release after a double click release is not skipped")
    },
    Row {
        given: &[
            stacks::at(9, Item::Stone, 5),
            stacks::at(20, Item::Stone, 7),
        ],
        given_cursor: stacks::held(Item::Dirt, 3),
        frames: &[
            &[
                Event::Key(SHIFT),
                over(9),
                Event::Press(LEFT),
                Event::Release(LEFT),
                Event::Press(LEFT),
                Event::Release(LEFT),
            ],
            &[],
        ],
        sent: &[
            on(ClickKind::QuickMove, 0, 9),
            on(ClickKind::QuickMove, 0, 20),
            on(ClickKind::QuickMove, 0, 36),
        ],
        entries: &[
            stacks::at(9, Item::Stone, 12),
            stacks::empty(20),
            stacks::empty(36),
        ],
        cursor: stacks::held(Item::Dirt, 3),
        ..row("a Shift double click moves each stack of the remembered item")
    },
    Row {
        screen: Screen::None,
        given: &[stacks::at(36, Item::Dirt, 5)],
        frames: &[&[Event::Key(KeyCode::KeyQ)]],
        sent: &[Sent::Hand(HandAction::DropOne)],
        entries: &[stacks::at(36, Item::Dirt, 4)],
        answers: None,
        ..row("the drop key outside a screen drops one item")
    },
    Row {
        screen: Screen::None,
        grab: CursorGrabMode::None,
        given: &[stacks::at(36, Item::Dirt, 4)],
        entries: &[stacks::at(36, Item::Dirt, 4)],
        frames: &[&[Event::Key(KeyCode::KeyQ), Event::Key(KeyCode::KeyF)]],
        answers: None,
        ..row("the hand keys with a released cursor do nothing")
    },
    Row {
        screen: Screen::None,
        given: &[stacks::at(36, Item::Dirt, 4)],
        frames: &[&[Event::Key(CONTROL), Event::Key(KeyCode::KeyQ)]],
        sent: &[Sent::Hand(HandAction::DropStack)],
        entries: &[stacks::empty(36)],
        answers: None,
        ..row("Control and the drop key outside a screen drop the stack")
    },
    Row {
        screen: Screen::None,
        given: &[
            stacks::at(36, Item::Dirt, 2),
            stacks::at(45, Item::Stone, 5),
        ],
        frames: &[&[Event::Key(KeyCode::KeyF)]],
        sent: &[Sent::Hand(HandAction::SwapHands)],
        entries: &[
            stacks::at(36, Item::Stone, 5),
            stacks::at(45, Item::Dirt, 2),
        ],
        answers: Some(ServerAnswers { slots: 2, ..AGREED }),
        ..row("the swap key outside a screen swaps the hands after the server answers")
    },
];

fn gui_point(pointer: Pointer) -> IVec2 {
    let entry_centre = |geometry: MenuGeometry, entry| {
        let window = geometry.window(640, 360);
        let corner = geometry.entry(entry).unwrap();
        IVec2::new(window.x + corner.x, window.y + corner.y) + 8
    };
    match pointer {
        Pointer::Entry(entry) => entry_centre(MenuGeometry::INVENTORY, entry),
        Pointer::ChestEntry(entry) => {
            entry_centre(MenuGeometry::of(MenuType::Generic9x3).unwrap(), entry)
        }
        Pointer::EmptyArea => WINDOW_CORNER + IVec2::new(7, 84),
        Pointer::Outside => WINDOW_CORNER + IVec2::new(-10, 20),
        Pointer::RecipeBookButton => WINDOW_CORNER + IVec2::new(104, 61),
    }
}

fn pick_up(world: &mut LocalWorld, entry: u16) {
    let viewer = world.viewer;
    world.app.world_mut().write_message(MenuInput {
        viewer,
        action: MenuAction::Click(stacks::click(ClickKind::PickUp, 0, Clicked::Entry(entry))),
    });
    world.app.update();
}

fn set_up(world: &mut LocalWorld, row: &Row) {
    world.switch_game_mode(GameMode::Creative);
    let cleared: Vec<Entry> = (9..=45).map(stacks::empty).collect();
    world.give(&cleared);
    pick_up(world, SCRATCH_ENTRY);
    world.give(&[stacks::empty(SCRATCH_ENTRY)]);
    world.app.insert_resource(row.screen);
    if row.given_cursor.is_some() {
        world.give(&[stacks::holding(SCRATCH_ENTRY, row.given_cursor)]);
        pick_up(world, SCRATCH_ENTRY);
        world.give(&[stacks::empty(SCRATCH_ENTRY)]);
    }
    world.give(row.given);
    test_support::round_trip(&mut world.app);
    world.switch_game_mode(row.game_mode);
}

fn collect_inputs(
    mut menu: MessageReader<MenuInput>,
    mut hand: MessageReader<HandInput>,
    mut sent: ResMut<SentInputs>,
) {
    for input in menu.read() {
        match input.action {
            MenuAction::Click(click) => sent.0.push(Sent::Click(click)),
            MenuAction::Close => sent.0.push(Sent::Close),
            _ => {}
        }
    }
    sent.0
        .extend(hand.read().map(|input| Sent::Hand(input.action)));
}

fn add_input(app: &mut App) {
    if !app.is_plugin_added::<InputPlugin>() {
        app.add_plugins(InputPlugin);
    }
    app.add_plugins((InventoryClicksPlugin, HandKeysPlugin))
        .insert_resource(GuiConfig {
            scale: GUI_SCALE,
            cursor: None,
            frozen_ticks: None,
            advanced_tooltips: false,
        })
        .init_resource::<SentInputs>()
        .add_systems(Last, collect_inputs);
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
        let key = |key_code, state, repeat| KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat,
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
                world.write_message(key(code, ButtonState::Pressed, false));
            }
            Event::Repeat(code) => {
                world.write_message(key(code, ButtonState::Pressed, true));
            }
            Event::Lift(code) => {
                world.write_message(key(code, ButtonState::Released, false));
            }
        }
    }
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(FRAME);
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
}

#[test]
fn each_gesture_gives_its_input_and_the_same_stacks_on_both_sides() {
    let mut world = LocalWorld::join(
        "screen-clicks",
        local_world::discard_thrown_items,
        add_input,
    );
    for row in ROWS {
        let mut cursor = world
            .app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>();
        cursor.single_mut(world.app.world_mut()).unwrap().grab_mode = row.grab;
        set_up(&mut world, row);
        world.clear_answers();
        world.app.world_mut().resource_mut::<SentInputs>().0.clear();
        for frame in row.frames {
            play(&mut world.app, frame);
        }
        let held: Vec<Event> = (world.app.world().resource::<ButtonInput<KeyCode>>())
            .get_pressed()
            .map(|&key| Event::Lift(key))
            .collect();
        play(&mut world.app, &held);
        assert_eq!(
            world.app.world().resource::<SentInputs>().0,
            row.sent,
            "{}: the inputs",
            row.name
        );
        world.assert_after_round_trip(row.name, row.answers, row.entries, row.cursor);
    }
    world.leave();
}

#[test]
fn a_menu_that_the_server_opens_between_a_press_and_its_release_takes_no_click() {
    let mut world = LocalWorld::join(
        "chest-between-press-and-release",
        container_menus::open_chest_on_command,
        add_input,
    );
    let case = Row {
        given_cursor: stacks::held(Item::Stone, 10),
        ..row("a menu that the server opens between a press and its release")
    };
    world.app.insert_resource(Screen::Menu);
    set_up(&mut world, &case);
    world.clear_answers();
    world.app.world_mut().resource_mut::<SentInputs>().0.clear();

    play(&mut world.app, &[over(10), Event::Press(LEFT)]);
    container_menus::open_chest(&mut world.app, world.viewer, 1);
    play(
        &mut world.app,
        &[Event::Move(Pointer::ChestEntry(0)), Event::Release(LEFT)],
    );
    assert_eq!(
        world.app.world().resource::<SentInputs>().0,
        [],
        "the release over the chest takes no click"
    );
    world.assert_after_round_trip(case.name, None, &[], stacks::held(Item::Stone, 10));
    world.leave();
}

#[test]
fn a_server_chest_opens_its_screen_takes_a_click_and_closes_it() {
    let mut world = LocalWorld::join(
        "chest-screen",
        container_menus::open_chest_on_command,
        add_input,
    );
    let viewer = world.viewer;
    world.app.insert_resource(Screen::None);
    let menu = container_menus::open_chest(&mut world.app, viewer, 1);
    assert_eq!(
        *world.app.world().resource::<Screen>(),
        Screen::Menu,
        "the screen after the open"
    );

    world.clear_answers();
    play(
        &mut world.app,
        &[Event::Move(Pointer::ChestEntry(0)), Event::Press(LEFT)],
    );
    play(&mut world.app, &[Event::Release(LEFT)]);
    assert_eq!(
        world.app.world().resource::<SentInputs>().0,
        [on(ClickKind::PickUp, 0, 0)],
        "the input of a left click on the first chest entry"
    );
    world.assert_after_round_trip(
        "a left click on the first chest entry",
        Some(AGREED),
        &[],
        stacks::held(Item::Stone, 10),
    );
    assert_eq!(
        container_menus::menu_stacks(world.app.world(), menu).first(),
        Some(&None),
        "the first chest entry after the click"
    );

    container_menus::close_on_server(&mut world.app);
    test_support::update_until(&mut world.app, |app| {
        *app.world().resource::<Screen>() == Screen::None
    });
    world.leave();
}

#[test]
fn a_server_close_of_the_inventory_menu_closes_its_screen() {
    let mut world = LocalWorld::join("inventory-server-close", |_| {}, add_input);
    world.app.insert_resource(Screen::None);
    play(&mut world.app, &[Event::Key(KeyCode::KeyE)]);
    assert_eq!(
        *world.app.world().resource::<Screen>(),
        Screen::Menu,
        "the screen after the inventory key"
    );
    let mut body = Vec::new();
    ClientboundContainerClose {
        container_id: VarInt(0),
    }
    .encode(&mut body)
    .unwrap();
    let viewer = world.viewer;
    world
        .app
        .world_mut()
        .trigger(Received::<ClientboundContainerClose>::new(
            viewer,
            body.into(),
        ));
    assert_eq!(
        *world.app.world().resource::<Screen>(),
        Screen::None,
        "the screen after the close of the inventory menu by the server"
    );
    world.leave();
}
