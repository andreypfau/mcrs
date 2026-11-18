use core::mem;
use core::time::Duration;
use std::collections::VecDeque;

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButtonInput, MouseWheel};
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
use mcrs_minecraft_menu_geometry::{CreativeHit, CreativeTabIndex, MenuGeometry};
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::chat_input;
use crate::close_menu_screen::close_menu_screen;
use crate::creative_clicks::CreativeClicks;
use crate::creative_grid::CreativeGrid;
use crate::creative_screen_edit::CreativeScreenEdit;
use crate::creative_screen_input::{self, CreativePointer, ScrollDrag, SearchPress};
use crate::creative_screen_plugin::CreativeContentSystems;
use crate::creative_screen_shown::shows_creative_screen;
use crate::cursor_grab;
use crate::drag_preview::DragPreview;
use crate::follow_open_menu::follow_open_menu;
use crate::gesture_frame::{self, GestureFrame};
use crate::hover::Hover;
use crate::item_key_codes;
use crate::mark_creative_screen::mark_creative_screen;
use crate::menu_gesture::MenuGesture;
use crate::menu_input;
use crate::menu_screen;
use crate::menu_stacks::MenuStacks;
use crate::modifiers::Modifiers;
use crate::release_rule::ReleaseRule;
use crate::scene::{self, GuiConfig, GuiSize};
use crate::search_input::SearchInput;
use crate::viewer::Viewer;
use crate::wheel_notches;

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
                    mark_creative_screen.after(CreativeContentSystems),
                    click_in_menu,
                )
                    .chain()
                    .after(ClientNetworkSystems::Receive)
                    .before(InventoryClientSet::Predict),
            )
            .add_observer(close_menu_screen);
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ScreenEvent {
    Press(MouseButton),
    Release(MouseButton),
    Key(Gesture),
    SelectSearchTab,
    Search(SearchInput),
    Close,
}

#[derive(Clone, Debug)]
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
    scroll: ScrollDrag,
    search: SearchPress,
}

#[derive(Clone, Debug)]
enum FrameEnd {
    Open,
    Closed,
    TabReleased(CreativeTabIndex),
    Search(SearchInput),
}

#[derive(SystemParam)]
struct Gestures<'w, 's> {
    mouse: MessageReader<'w, 's, MouseButtonInput>,
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    wheel: MessageReader<'w, 's, MouseWheel>,
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

    fn creative(&self) -> Option<CreativePointer> {
        let (Ok(window), Some(config)) = (self.window.single(), self.config.as_deref()) else {
            return None;
        };
        Some(creative_screen_input::creative_pointer(window, config))
    }

    fn notches(&self, wheel: &MouseWheel) -> f64 {
        let scale = (self.window.single()).map_or(1.0, |window| window.scale_factor());
        f64::from(wheel.y) * wheel_notches::notches_per_unit(wheel.unit, scale)
    }

    fn hover(&self, geometry: Option<MenuGeometry>, creative: Option<CreativeGrid>) -> Hover {
        let outside = Hover::Menu(Clicked::Outside);
        let (Ok(window), Some(config)) = (self.window.single(), self.config.as_deref()) else {
            return outside;
        };
        let size = GuiSize::of(scene::window_framebuffer(window), config.scale);
        let Some(pointer) = scene::gui_pointer(window, config, size.scale).map(scene::gui_point)
        else {
            return outside;
        };
        if let Some(creative) = creative {
            return Hover::Creative(creative.hit(size.width, size.height, pointer));
        }
        let Some(geometry) = geometry else {
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

    fn shows_creative_screen(&self, screen: Screen) -> bool {
        self.player
            .single()
            .is_ok_and(|(_, _, menus, abilities)| shows_creative_screen(screen, menus, abilities))
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

pub(crate) fn key_event(key: KeyCode) -> Option<ScreenEvent> {
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
    fn reset(&mut self, gestures: &mut Gestures) {
        gestures.mouse.clear();
        gestures.keyboard.clear();
        gestures.wheel.clear();
        self.events.clear();
        self.pointer = None;
        self.gesture = MenuGesture::default();
        self.scroll = ScrollDrag::Idle;
        self.search = SearchPress::None;
    }

    fn is_idle(&self, gestures: &Gestures) -> bool {
        gestures.mouse.is_empty()
            && gestures.keyboard.is_empty()
            && gestures.wheel.is_empty()
            && self.events.is_empty()
            && self.gesture.open_drag().is_none()
            && self.scroll == ScrollDrag::Idle
            && !matches!(self.search, SearchPress::Held(_))
    }

    fn scroll(
        &mut self,
        wheel: &mut MessageReader<MouseWheel>,
        pointer: &ScreenPointer,
        creative: bool,
        tabs: &mut CreativeScreenEdit,
    ) -> bool {
        let notches: f64 = wheel.read().map(|wheel| pointer.notches(wheel)).sum();
        if creative && notches != 0.0 {
            tabs.wheel(notches);
        }
        let position = pointer.position();
        let moved = mem::replace(&mut self.pointer, position) != position;
        if moved
            && creative
            && let Some(at) = pointer.creative()
        {
            creative_screen_input::follow_pointer(self.scroll, self.search, at, tabs);
        }
        moved
    }

    fn collect(&mut self, gestures: &mut Gestures, hover: Hover, creative: Option<CreativeGrid>) {
        let mut searches = creative.is_some_and(CreativeGrid::searches);
        let key_modifiers = menu_input::modifiers(&gestures.keys);
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
            if event.state != ButtonState::Pressed {
                continue;
            }
            let screen_event = match event.key_code {
                KeyCode::Escape => ScreenEvent::Close,
                _ if searches => ScreenEvent::Search(SearchInput::Key {
                    input: event.clone(),
                    modifiers: key_modifiers,
                }),
                chat_input::CHAT if creative.is_some() => {
                    searches = true;
                    ScreenEvent::SelectSearchTab
                }
                code => match key_event(code) {
                    Some(screen_event) => screen_event,
                    None => continue,
                },
            };
            self.events.push_back(QueuedEvent {
                event: screen_event,
                hover,
                modifiers,
                at,
            });
        }
    }
}

fn drain_without_menu(events: &mut VecDeque<QueuedEvent>) -> FrameEnd {
    if (events.drain(..)).any(|queued| matches!(queued.event, ScreenEvent::Close)) {
        FrameEnd::Closed
    } else {
        FrameEnd::Open
    }
}

fn play(
    frame: &mut GestureFrame,
    events: &mut VecDeque<QueuedEvent>,
    scroll: &mut ScrollDrag,
    search: &mut SearchPress,
) -> FrameEnd {
    while let Some(queued) = events.pop_front() {
        let sent = frame.sent.len();
        let QueuedEvent {
            event,
            hover,
            modifiers,
            at,
        } = queued;
        if matches!(event, ScreenEvent::Release(MouseButton::Left)) {
            *scroll = ScrollDrag::Idle;
            if let SearchPress::Held(pressed) = *search {
                *search = SearchPress::Released(pressed);
            }
            if let Some(tab) = hover.tab() {
                return select_tab(frame, scroll, tab);
            }
        }
        match event {
            ScreenEvent::Close => {
                events.clear();
                return FrameEnd::Closed;
            }
            ScreenEvent::SelectSearchTab => {
                return select_tab(frame, scroll, CreativeTabIndex::SEARCH);
            }
            ScreenEvent::Press(button) => {
                if button == MouseButton::Left
                    && matches!(hover, Hover::Creative(CreativeHit::ScrollBar))
                    && frame.grid.is_some_and(CreativeGrid::can_scroll)
                {
                    *scroll = ScrollDrag::Dragging;
                }
                frame.press(button, at, hover, modifiers);
                if button == MouseButton::Left {
                    if matches!(hover, Hover::Creative(CreativeHit::SearchBox)) {
                        return FrameEnd::Search(creative_screen_input::search_press(
                            search, at, modifiers,
                        ));
                    }
                    *search = SearchPress::None;
                }
                if matches!(hover, Hover::Creative(CreativeHit::GridSlot(_)))
                    && frame.grid.is_some_and(CreativeGrid::searches)
                {
                    return FrameEnd::Search(SearchInput::SelectAll);
                }
            }
            ScreenEvent::Release(button) => {
                frame.release(button, hover, modifiers);
            }
            ScreenEvent::Key(gesture) => {
                frame.click(hover, gesture, modifiers);
            }
            ScreenEvent::Search(input) => {
                if !creative_screen_input::swaps_from_search(frame, &input, hover, modifiers) {
                    return FrameEnd::Search(input);
                }
            }
        }
        if frame.sent.len() > sent {
            break;
        }
    }
    FrameEnd::Open
}

fn select_tab(
    frame: &mut GestureFrame,
    scroll: &mut ScrollDrag,
    tab: CreativeTabIndex,
) -> FrameEnd {
    *scroll = ScrollDrag::Idle;
    frame.gesture.release = ReleaseRule::NoClick;
    FrameEnd::TabReleased(tab)
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
    CreativeScreenEdit<'w>,
);

fn click_in_menu(
    mut gestures: Gestures,
    pointer: ScreenPointer,
    stacks: LocalStacks,
    mut items: Option<Stacks>,
    mut world_and_outputs: ParamSet<((Option<MenuTargets>, CreativeClicks), Outputs)>,
    mut state: Local<ClickState>,
    mut menu_stacks: Local<MenuStacks>,
) {
    let (screen, _, _, mut drag_preview, _) = world_and_outputs.p1();
    let current = screen.as_deref().copied().unwrap_or_default();
    let open = stacks.open_menu();
    let menu = open.map(|open| open.menu);
    let last = mem::replace(&mut state.last_screen, current);
    let last_menu = mem::replace(&mut state.last_menu, menu);
    if last != Screen::Menu || current != Screen::Menu || last_menu != menu {
        state.reset(&mut gestures);
        drag_preview.clear();
        return;
    }
    if state.is_idle(&gestures) {
        return;
    }
    let geometry = open.and_then(|open| MenuGeometry::of_layout(open.layout));
    let shown = stacks.shows_creative_screen(current);
    let clicks = world_and_outputs.p0().1;
    let creative = clicks.grid().filter(|_| shown);
    let hover = pointer.hover(geometry, creative);
    state.collect(&mut gestures, hover, creative);
    let mut tabs = world_and_outputs.p1().4;
    let moved = state.scroll(&mut gestures.wheel, &pointer, shown, &mut tabs);
    let ClickState {
        last_screen,
        events,
        sent,
        preview,
        gesture,
        scroll,
        search,
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
            let mut followed = false;
            loop {
                let (mut targets, creative) = world_and_outputs.p0();
                let view =
                    (targets.as_mut()).and_then(|targets| targets.read(viewer.entity, cursor));
                let mut frame = GestureFrame {
                    viewer: &viewer,
                    targets: view.as_ref(),
                    items: items.as_mut(),
                    grid: creative.grid().filter(|_| shown),
                    gesture: &mut *gesture,
                    sent: &mut *sent,
                };
                if !mem::replace(&mut followed, true) {
                    frame.follow_pointer(hover, moved, preview);
                }
                let end = play(&mut frame, events, scroll, search);
                frame.project_preview(preview);
                let FrameEnd::Search(input) = end else {
                    break end;
                };
                (world_and_outputs.p1().4).edit_search(&input, pointer.creative());
                if !sent.is_empty() {
                    break FrameEnd::Open;
                }
            }
        }
        None => {
            *gesture = MenuGesture::default();
            preview.clear();
            drain_without_menu(events)
        }
    };
    let (screen, mut cursor, mut inputs, mut drag_preview, mut tabs) = world_and_outputs.p1();
    for input in sent.drain(..) {
        inputs.write(input);
    }
    if let FrameEnd::TabReleased(tab) = end {
        tabs.select(tab);
    }
    if matches!(end, FrameEnd::Closed) {
        *scroll = ScrollDrag::Idle;
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
