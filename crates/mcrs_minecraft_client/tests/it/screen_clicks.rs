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
use mcrs_minecraft_player_holders::HandAction;

use crate::local_world::LocalWorld;
use crate::stacks::{self, AGREED, Entry, Item, Stack};

const GUI_SCALE: u32 = 2;
const SCRATCH_ENTRY: u16 = 9;
const WINDOW_CORNER: IVec2 = IVec2::new((640 - 176) / 2, (360 - 166) / 2);

#[derive(Clone, Copy, Debug)]
enum Pointer {
    Entry(u16),
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
        screen: Screen::Inventory,
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
    WINDOW_CORNER
        + match pointer {
            Pointer::Entry(entry) => {
                let corner = MenuGeometry::INVENTORY.entry(entry).unwrap();
                IVec2::new(corner.x, corner.y) + 8
            }
            Pointer::EmptyArea => IVec2::new(7, 84),
            Pointer::Outside => IVec2::new(-10, 20),
            Pointer::RecipeBookButton => IVec2::new(104, 61),
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
    if row.given_cursor.is_some() {
        world.give(&[stacks::holding(SCRATCH_ENTRY, row.given_cursor)]);
        pick_up(world, SCRATCH_ENTRY);
    }
    world.give(&[stacks::empty(SCRATCH_ENTRY)]);
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
    app.update();
    app.world_mut().run_schedule(FixedUpdate);
}

#[test]
fn each_gesture_gives_its_input_and_the_same_stacks_on_both_sides() {
    let mut world = LocalWorld::join("screen-clicks", |_| {}, add_input);
    for row in ROWS {
        world.app.insert_resource(row.screen);
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
