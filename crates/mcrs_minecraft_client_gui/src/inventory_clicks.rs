use core::mem;
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
use mcrs_minecraft_container_click::{
    ButtonBinding, Click, ClickInput, Clicked, Gesture, StackPresence,
};
use mcrs_minecraft_engine_inventory_menu::{MenuParts, Menus};
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_client::{InventoryClientSet, MenuAction, MenuInput};
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_menu_geometry::MenuGeometry;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::close_menu_screen::close_menu_screen;
use crate::cursor_grab;
use crate::follow_open_menu::follow_open_menu;
use crate::item_key_codes;
use crate::menu_screen;
use crate::menu_stacks::MenuStacks;
use crate::scene::{self, GuiConfig, GuiSize};

const INVENTORY: KeyCode = KeyCode::KeyE;

pub struct InventoryClicksPlugin;

impl Plugin for InventoryClicksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ReleaseRule {
    #[default]
    NoClick,
    Click,
    Drag {
        button: u8,
    },
}

#[derive(Clone, Copy, Debug)]
enum ScreenEvent {
    Press(MouseButton),
    Release(MouseButton),
    Key(Gesture),
    Close,
}

#[derive(Clone, Copy, Debug)]
struct Modifiers {
    shift: bool,
    control: bool,
}

#[derive(Clone, Copy, Debug)]
struct QueuedEvent {
    event: ScreenEvent,
    hover: Hover,
    modifiers: Modifiers,
}

#[derive(Default)]
struct ClickState {
    last_screen: Screen,
    last_menu: Option<Entity>,
    release: ReleaseRule,
    events: VecDeque<QueuedEvent>,
}

#[derive(SystemParam)]
struct Gestures<'w, 's> {
    mouse: MessageReader<'w, 's, MouseButtonInput>,
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    keys: Res<'w, ButtonInput<KeyCode>>,
}

#[derive(SystemParam)]
struct ScreenPointer<'w, 's> {
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    config: Option<Res<'w, GuiConfig>>,
}

#[derive(Clone, Copy, Debug)]
enum Hover {
    RecipeBookButton,
    Menu(Clicked),
}

impl Hover {
    fn clicked(self) -> Clicked {
        match self {
            Self::RecipeBookButton => Clicked::Nowhere,
            Self::Menu(clicked) => clicked,
        }
    }
}

impl ScreenPointer<'_, '_> {
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

struct Viewer<'a> {
    entity: Entity,
    creative: bool,
    stacks: &'a MenuStacks,
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

    fn viewer<'a>(&self, open: OpenMenu, stacks: &'a mut MenuStacks) -> Viewer<'a> {
        stacks.collect(open.layout, open.parts, open.holders.cursor, |slot| {
            self.holders
                .get(slot.holder)
                .ok()?
                .get(usize::from(slot.index))
        });
        Viewer {
            entity: open.viewer,
            creative: open.creative,
            stacks,
        }
    }
}

fn presence(stack: Option<Entity>) -> StackPresence {
    match stack {
        Some(_) => StackPresence::Stack,
        None => StackPresence::Empty,
    }
}

impl Viewer<'_> {
    fn input(&self, gesture: Gesture, clicked: Clicked, modifiers: Modifiers) -> ClickInput {
        ClickInput {
            gesture,
            shift: modifiers.shift,
            control: modifiers.control,
            creative: self.creative,
            cursor: presence(self.stacks.carried()),
            target: match clicked {
                Clicked::Entry(menu) => presence(self.stacks.get(menu)),
                Clicked::Outside | Clicked::Nowhere => StackPresence::Empty,
            },
        }
    }

    fn click(&self, clicked: Clicked, input: ClickInput) -> Option<MenuInput> {
        Click::from_input(clicked, input).map(|click| MenuInput {
            viewer: self.entity,
            action: MenuAction::Click(click),
        })
    }
}

fn button_number(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Right => Some(1),
        MouseButton::Middle => Some(2),
        MouseButton::Back => Some(4),
        MouseButton::Forward => Some(5),
        MouseButton::Other(_) => None,
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

fn button_binding(button: MouseButton) -> ButtonBinding {
    match button {
        MouseButton::Middle => ButtonBinding::PickItem,
        _ => ButtonBinding::Other,
    }
}

impl ReleaseRule {
    fn press(
        &mut self,
        button: MouseButton,
        viewer: &Viewer,
        hover: Hover,
        modifiers: Modifiers,
    ) -> Option<MenuInput> {
        if button == MouseButton::Left && matches!(hover, Hover::RecipeBookButton) {
            *self = Self::NoClick;
            return None;
        }
        let clicked = hover.clicked();
        let number = button_number(button)?;
        let binding = button_binding(button);
        let gesture = Gesture::Press {
            button: number,
            binding,
        };
        let input = viewer.input(gesture, clicked, modifiers);
        let click = viewer.click(clicked, input);
        let opens_drag = input.cursor == StackPresence::Stack
            && clicked != Clicked::Nowhere
            && (matches!(button, MouseButton::Left | MouseButton::Right)
                || (binding == ButtonBinding::PickItem && input.creative));
        *self = match (&click, *self) {
            (Some(_), _) => Self::NoClick,
            (None, Self::Drag { .. }) => *self,
            (None, _) if opens_drag => Self::Drag { button: number },
            (None, _) => Self::Click,
        };
        click
    }

    fn release(
        &mut self,
        button: MouseButton,
        viewer: &Viewer,
        clicked: Clicked,
        modifiers: Modifiers,
    ) -> Option<MenuInput> {
        let number = button_number(button)?;
        match mem::take(self) {
            Self::NoClick => None,
            Self::Drag { button: open } if open != number => None,
            Self::Drag { .. } | Self::Click => {
                let gesture = Gesture::Release {
                    button: number,
                    binding: button_binding(button),
                };
                viewer.click(clicked, viewer.input(gesture, clicked, modifiers))
            }
        }
    }
}

impl QueuedEvent {
    fn input(self, release: &mut ReleaseRule, viewer: &Viewer) -> Option<MenuInput> {
        let clicked = self.hover.clicked();
        match self.event {
            ScreenEvent::Press(button) => release.press(button, viewer, self.hover, self.modifiers),
            ScreenEvent::Release(button) => {
                release.release(button, viewer, clicked, self.modifiers)
            }
            ScreenEvent::Key(gesture) => {
                viewer.click(clicked, viewer.input(gesture, clicked, self.modifiers))
            }
            ScreenEvent::Close => None,
        }
    }
}

impl ClickState {
    fn collect(
        &mut self,
        gestures: &mut Gestures,
        pointer: &ScreenPointer,
        geometry: Option<MenuGeometry>,
    ) {
        if gestures.mouse.is_empty() && gestures.keyboard.is_empty() {
            return;
        }
        let hover = pointer.hover(geometry);
        let modifiers = Modifiers {
            shift: gestures.keys.any_pressed(item_key_codes::SHIFT),
            control: gestures.keys.any_pressed(item_key_codes::CONTROL),
        };
        for event in gestures.mouse.read() {
            if button_number(event.button).is_some() {
                self.events.push_back(QueuedEvent {
                    event: match event.state {
                        ButtonState::Pressed => ScreenEvent::Press(event.button),
                        ButtonState::Released => ScreenEvent::Release(event.button),
                    },
                    hover,
                    modifiers,
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
                });
            }
        }
    }

    fn reset(&mut self) {
        self.release = ReleaseRule::NoClick;
        self.events.clear();
    }
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

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each access is its own parameter"
)]
fn click_in_menu(
    mut gestures: Gestures,
    screen: Option<ResMut<Screen>>,
    pointer: ScreenPointer,
    stacks: LocalStacks,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut inputs: MessageWriter<MenuInput>,
    mut state: Local<ClickState>,
    mut menu_stacks: Local<MenuStacks>,
) {
    let current = screen.as_deref().copied().unwrap_or_default();
    let open = stacks.open_menu();
    let menu = open.map(|open| open.menu);
    let last = mem::replace(&mut state.last_screen, current);
    let last_menu = mem::replace(&mut state.last_menu, menu);
    let Some(mut screen) =
        screen.filter(|_| last == Screen::Menu && current == Screen::Menu && last_menu == menu)
    else {
        gestures.mouse.clear();
        gestures.keyboard.clear();
        state.reset();
        return;
    };
    let geometry = open.and_then(|open| MenuGeometry::of_layout(open.layout));
    state.collect(&mut gestures, &pointer, geometry);
    if state.events.is_empty() {
        return;
    }
    let viewer = open.map(|open| stacks.viewer(open, &mut menu_stacks));
    let ClickState {
        last_screen,
        release,
        events,
        ..
    } = &mut *state;
    while let Some(queued) = events.pop_front() {
        if let ScreenEvent::Close = queued.event {
            if let Some(player) = stacks.player() {
                inputs.write(MenuInput {
                    viewer: player,
                    action: MenuAction::Close,
                });
            }
            *screen = Screen::None;
            *last_screen = Screen::None;
            if let Ok(mut cursor) = cursor.single_mut() {
                cursor_grab::grab(&mut cursor, true);
            }
            *release = ReleaseRule::NoClick;
            events.clear();
            return;
        }
        let Some(viewer) = &viewer else {
            *release = ReleaseRule::NoClick;
            continue;
        };
        if let Some(input) = queued.input(release, viewer) {
            inputs.write(input);
            break;
        }
    }
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
