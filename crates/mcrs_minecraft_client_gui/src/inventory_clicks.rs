use core::mem;
use core::time::Duration;
use std::collections::VecDeque;

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseButtonInput;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_container_click::{Clicked, Gesture};
use mcrs_minecraft_engine_inventory_click::MenuTargets;
use mcrs_minecraft_engine_inventory_menu::{MenuParts, Menus};
use mcrs_minecraft_engine_item::{SlotRef, Slots, Stacks};
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_client::{InventoryClientSet, MenuAction, MenuInput};
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_menu_geometry::MenuGeometry;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::close_menu_screen::close_menu_screen;
use crate::cursor_grab;
use crate::drag_preview::DragPreview;
use crate::follow_open_menu::follow_open_menu;
use crate::gesture_frame::{self, GestureFrame};
use crate::hover::Hover;
use crate::item_key_codes;
use crate::menu_gesture::MenuGesture;
use crate::menu_screen;
use crate::menu_stacks::MenuStacks;
use crate::modifiers::Modifiers;
use crate::scene::{self, GuiConfig, GuiSize};
use crate::viewer::Viewer;

const INVENTORY: KeyCode = KeyCode::KeyE;

pub struct InventoryClicksPlugin;

impl Plugin for InventoryClicksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DragPreview>()
            .add_systems(
                Update,
                (
                    open_inventory.run_if(input_just_pressed(INVENTORY)),
                    follow_open_menu,
                    click_in_menu,
                )
                    .chain()
                    .after(ClientNetworkSystems::Receive)
                    .before(InventoryClientSet::Predict),
            )
            .add_observer(close_menu_screen);
    }
}

#[derive(Clone, Copy, Debug)]
enum ScreenEvent {
    Press(MouseButton),
    Release(MouseButton),
    Key(Gesture),
    Close,
}

#[derive(Clone, Copy, Debug)]
struct QueuedEvent {
    event: ScreenEvent,
    hover: Hover,
    modifiers: Modifiers,
    at: Duration,
}

#[derive(Default)]
struct ClickState {
    last_screen: Screen,
    last_menu: Option<Entity>,
    pointer: Option<Vec2>,
    events: VecDeque<QueuedEvent>,
    sent: Vec<MenuInput>,
    preview: DragPreview,
    gesture: MenuGesture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameEnd {
    Open,
    Closed,
}

#[derive(SystemParam)]
struct Gestures<'w, 's> {
    mouse: MessageReader<'w, 's, MouseButtonInput>,
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    time: Res<'w, Time<Real>>,
}

#[derive(SystemParam)]
struct ScreenPointer<'w, 's> {
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    config: Option<Res<'w, GuiConfig>>,
}

impl ScreenPointer<'_, '_> {
    fn position(&self) -> Option<Vec2> {
        self.window.single().ok()?.cursor_position()
    }

    fn hover(&self, geometry: Option<MenuGeometry>) -> Hover {
        let outside = Hover::Menu(Clicked::Outside);
        let (Ok(window), Some(config), Some(geometry)) =
            (self.window.single(), self.config.as_deref(), geometry)
        else {
            return outside;
        };
        let size = GuiSize::of(scene::window_framebuffer(window), config.scale);
        let Some(pointer) = scene::gui_pointer(window, config, size.scale).map(scene::gui_point)
        else {
            return outside;
        };
        let corner = geometry.window(size.width, size.height);
        if geometry.is_inventory()
            && menu_screen::recipe_book_button(corner).contains_point(pointer)
        {
            Hover::RecipeBookButton
        } else {
            Hover::Menu(geometry.clicked_at(corner, pointer))
        }
    }
}

#[derive(SystemParam)]
struct LocalStacks<'w, 's> {
    player: Query<
        'w,
        's,
        (
            Entity,
            &'static PlayerHolders,
            &'static Menus,
            Option<&'static Abilities>,
        ),
        With<Player>,
    >,
    menus: Query<'w, 's, (&'static MenuLayout, &'static MenuParts)>,
    holders: Query<'w, 's, &'static Slots>,
}

#[derive(Clone, Copy)]
struct OpenMenu<'a> {
    viewer: Entity,
    creative: bool,
    holders: &'a PlayerHolders,
    menu: Entity,
    layout: &'a MenuLayout,
    parts: &'a MenuParts,
}

impl LocalStacks<'_, '_> {
    fn player(&self) -> Option<Entity> {
        self.player.single().ok().map(|(entity, ..)| entity)
    }

    fn open_menu(&self) -> Option<OpenMenu<'_>> {
        let (viewer, holders, menus, abilities) = self.player.single().ok()?;
        let menu = menus.open()?;
        let (layout, parts) = self.menus.get(menu).ok()?;
        Some(OpenMenu {
            viewer,
            creative: abilities.is_some_and(|abilities| abilities.instant_build),
            holders,
            menu,
            layout,
            parts,
        })
    }

    fn viewer<'a>(&'a self, open: OpenMenu<'a>, stacks: &'a mut MenuStacks) -> Viewer<'a> {
        stacks.collect(open.layout, open.parts, open.holders.cursor, |slot| {
            self.holders
                .get(slot.holder)
                .ok()?
                .get(usize::from(slot.index))
        });
        Viewer {
            entity: open.viewer,
            menu: open.menu,
            creative: open.creative,
            stacks,
        }
    }
}

fn key_event(key: KeyCode) -> Option<ScreenEvent> {
    let gesture = match key {
        INVENTORY | KeyCode::Escape => return Some(ScreenEvent::Close),
        item_key_codes::SWAP_OFFHAND => Gesture::OffhandKey,
        item_key_codes::DROP => Gesture::DropKey,
        _ => {
            let index = item_key_codes::HOTBAR
                .iter()
                .position(|&hotbar| hotbar == key)?;
            let number = u8::try_from(index).ok()?.checked_add(1)?;
            Gesture::HotbarKey(number)
        }
    };
    Some(ScreenEvent::Key(gesture))
}

impl ClickState {
    fn collect(&mut self, gestures: &mut Gestures, hover: Hover) {
        let modifiers = Modifiers {
            shift: gestures.keys.any_pressed(item_key_codes::SHIFT),
            control: gestures.keys.any_pressed(item_key_codes::CONTROL),
        };
        let at = gestures.time.elapsed();
        for event in gestures.mouse.read() {
            if gesture_frame::button_number(event.button).is_some() {
                self.events.push_back(QueuedEvent {
                    event: match event.state {
                        ButtonState::Pressed => ScreenEvent::Press(event.button),
                        ButtonState::Released => ScreenEvent::Release(event.button),
                    },
                    hover,
                    modifiers,
                    at,
                });
            }
        }
        for event in gestures.keyboard.read() {
            if event.state == ButtonState::Pressed
                && let Some(screen_event) = key_event(event.key_code)
            {
                self.events.push_back(QueuedEvent {
                    event: screen_event,
                    hover,
                    modifiers,
                    at,
                });
            }
        }
    }
}

fn play(frame: &mut GestureFrame, events: &mut VecDeque<QueuedEvent>) -> FrameEnd {
    while let Some(queued) = events.pop_front() {
        let sent = frame.sent.len();
        match queued.event {
            ScreenEvent::Close => {
                events.clear();
                return FrameEnd::Closed;
            }
            ScreenEvent::Press(button) => {
                frame.press(button, queued.at, queued.hover, queued.modifiers);
            }
            ScreenEvent::Release(button) => {
                frame.release(button, queued.hover, queued.modifiers);
            }
            ScreenEvent::Key(gesture) => {
                frame.click(queued.hover.clicked(), gesture, queued.modifiers);
            }
        }
        if frame.sent.len() > sent {
            break;
        }
    }
    FrameEnd::Open
}

fn open_inventory(
    mut screen: ResMut<Screen>,
    joined: Query<(), With<JoinedGame>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if *screen == Screen::None && !joined.is_empty() {
        *screen = Screen::Menu;
        cursor_grab::grab(&mut cursor, false);
    }
}

type Outputs<'w, 's> = (
    Option<ResMut<'w, Screen>>,
    Query<'w, 's, &'static mut CursorOptions, With<PrimaryWindow>>,
    MessageWriter<'w, MenuInput>,
    ResMut<'w, DragPreview>,
);

fn click_in_menu(
    mut gestures: Gestures,
    pointer: ScreenPointer,
    stacks: LocalStacks,
    mut items: Option<Stacks>,
    mut world_and_outputs: ParamSet<(Option<MenuTargets>, Outputs)>,
    mut state: Local<ClickState>,
    mut menu_stacks: Local<MenuStacks>,
) {
    let (screen, _, _, mut drag_preview) = world_and_outputs.p1();
    let current = screen.as_deref().copied().unwrap_or_default();
    let open = stacks.open_menu();
    let menu = open.map(|open| open.menu);
    let last = mem::replace(&mut state.last_screen, current);
    let last_menu = mem::replace(&mut state.last_menu, menu);
    if last != Screen::Menu || current != Screen::Menu || last_menu != menu {
        gestures.mouse.clear();
        gestures.keyboard.clear();
        state.events.clear();
        state.pointer = None;
        state.gesture = MenuGesture::default();
        drag_preview.clear();
        return;
    }
    let has_input = !gestures.mouse.is_empty() || !gestures.keyboard.is_empty();
    if !has_input && state.events.is_empty() && state.gesture.open_drag().is_none() {
        return;
    }
    let hover = pointer.hover(open.and_then(|open| MenuGeometry::of_layout(open.layout)));
    if has_input {
        state.collect(&mut gestures, hover);
    }
    let position = pointer.position();
    let moved = mem::replace(&mut state.pointer, position) != position;
    let ClickState {
        last_screen,
        events,
        sent,
        preview,
        gesture,
        ..
    } = &mut *state;
    sent.clear();
    let end = match open {
        Some(open) => {
            let cursor = SlotRef {
                holder: open.holders.cursor,
                index: 0,
            };
            let viewer = stacks.viewer(open, &mut menu_stacks);
            let mut targets = world_and_outputs.p0();
            let view = (targets.as_mut()).and_then(|targets| targets.read(viewer.entity, cursor));
            let mut frame = GestureFrame {
                viewer: &viewer,
                targets: view.as_ref(),
                items: items.as_mut(),
                gesture,
                sent,
            };
            frame.follow_pointer(hover, moved, preview);
            let end = play(&mut frame, events);
            frame.project_preview(preview);
            end
        }
        None => {
            *gesture = MenuGesture::default();
            preview.clear();
            let closed =
                (events.drain(..)).any(|queued| matches!(queued.event, ScreenEvent::Close));
            if closed {
                FrameEnd::Closed
            } else {
                FrameEnd::Open
            }
        }
    };
    let (screen, mut cursor, mut inputs, mut drag_preview) = world_and_outputs.p1();
    for input in sent.drain(..) {
        inputs.write(input);
    }
    if end == FrameEnd::Closed {
        preview.clear();
        if let Some(player) = stacks.player() {
            inputs.write(MenuInput {
                viewer: player,
                action: MenuAction::Close,
            });
        }
        if let Some(mut screen) = screen {
            *screen = Screen::None;
        }
        *last_screen = Screen::None;
        if let Ok(mut cursor) = cursor.single_mut() {
            cursor_grab::grab(&mut cursor, true);
        }
        *gesture = MenuGesture::default();
    }
    mem::swap(&mut *drag_preview, preview);
}

#[cfg(test)]
mod tests {
    use bevy::window::CursorGrabMode;

    use super::*;

    fn key_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Screen>()
            .add_systems(Update, open_inventory);
        app.world_mut().spawn(JoinedGame {
            player_id: 1,
            dimensions: Vec::new(),
        });
        app.world_mut().spawn((
            PrimaryWindow,
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                ..default()
            },
        ));
        app
    }

    fn press(app: &mut App, key: KeyCode) -> (Screen, CursorGrabMode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.clear();
        let grab_mode = app
            .world_mut()
            .query::<&CursorOptions>()
            .single(app.world())
            .unwrap()
            .grab_mode;
        (*app.world().resource::<Screen>(), grab_mode)
    }

    #[test]
    fn e_opens_the_inventory() {
        let mut app = key_app();
        assert_eq!(
            press(&mut app, KeyCode::KeyE),
            (Screen::Menu, CursorGrabMode::None)
        );
    }

    #[test]
    fn e_does_nothing_before_the_join() {
        let mut app = key_app();
        let joined = app
            .world_mut()
            .query_filtered::<Entity, With<JoinedGame>>()
            .single(app.world())
            .unwrap();
        app.world_mut().despawn(joined);
        assert_eq!(
            press(&mut app, KeyCode::KeyE),
            (Screen::None, CursorGrabMode::Locked)
        );
    }
}
