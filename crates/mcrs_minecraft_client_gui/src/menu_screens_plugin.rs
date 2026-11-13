use bevy::ecs::system::SystemParam;
use bevy::input::InputSystems;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::{
    GuiExtent, GuiInput, GuiPoint, Widget, WidgetEvent, WidgetId, Widgets,
};
use mcrs_minecraft_client_world::screen::{ControlsParent, Screen};

use crate::accessibility_settings_screen::{
    self, accessibility_settings_screen, fov_effects_tooltip,
};
use crate::controls_screen::{self, controls_screen};
use crate::cursor_grab::grab;
use crate::language::ClientLanguage;
use crate::menu::{Menu, MenuBackground};
use crate::menu_event::MenuEvent;
use crate::menu_input::{menu_clock, modifiers};
use crate::menu_input_systems::MenuInputSystems;
use crate::menu_options_systems::MenuOptionsSystems;
use crate::mouse_settings_screen::{self, mouse_settings_screen};
use crate::option_sub_screen;
use crate::options_screen::{self, options_screen};
use crate::pause_screen::{self, pause_screen};
use crate::scene::{GuiAtlas, GuiScene, GuiSize, window_framebuffer};
use crate::video_settings_screen::{self, video_settings_screen, wheel_gui_scale};

/// The pause screen, the options screen and the option sub-screens, each a [`Screen`] whose
/// [`Menu`] follows the screen, the GUI size, the options and the language. Escape in the game
/// opens the pause screen. Needs [`crate::menu_plugin::MenuPlugin`].
pub struct MenuScreensPlugin {
    /// Whether the client plays on a server it runs itself, which names the last button of the
    /// pause screen (MENU-PAUSE-005).
    pub local_world: bool,
}

#[derive(Resource)]
struct LastButtonKey(&'static str);

impl Plugin for MenuScreensPlugin {
    fn build(&self, app: &mut App) {
        let last_key = if self.local_world {
            "menu.returnToMenu"
        } else {
            "menu.disconnect"
        };
        app.insert_resource(LastButtonKey(last_key)).add_systems(
            PreUpdate,
            (
                (
                    open_pause_screen.run_if(input_just_pressed(KeyCode::Escape)),
                    ctrl_wheel_gui_scale
                        .run_if(resource_equals(Screen::VideoSettings))
                        .in_set(MenuOptionsSystems),
                    project_menu.run_if(
                        resource_exists::<GuiAtlas>
                            .and_then(resource_exists::<ClientLanguage>)
                            .and_then(resource_changed::<Screen>.or_else(resource_exists::<Menu>)),
                    ),
                )
                    .chain()
                    .after(InputSystems)
                    .before(MenuInputSystems),
                (
                    write_menu_options.in_set(MenuOptionsSystems),
                    show_pending_values,
                    follow_menu_events,
                )
                    .chain()
                    .after(MenuInputSystems)
                    .run_if(resource_exists::<Menu>),
            ),
        );
    }
}

/// The screen that Escape and "Done" open (WID-SCR-002); the game for the pause screen.
fn parent(screen: Screen) -> Screen {
    match screen {
        Screen::Options => Screen::Pause,
        Screen::VideoSettings
        | Screen::AccessibilitySettings
        | Screen::Controls(ControlsParent::Options) => Screen::Options,
        Screen::Controls(ControlsParent::AccessibilitySettings) => Screen::AccessibilitySettings,
        Screen::MouseSettings(parent) => Screen::Controls(parent),
        Screen::None
        | Screen::Inventory
        | Screen::GameModeSwitcher(_)
        | Screen::Chat
        | Screen::Pause => Screen::None,
    }
}

/// Whether a re-arrange keeps the widgets, their values and the focus (MENU-PROP-001).
fn keeps_widgets(screen: Screen) -> bool {
    matches!(
        screen,
        Screen::Options
            | Screen::VideoSettings
            | Screen::AccessibilitySettings
            | Screen::Controls(_)
            | Screen::MouseSettings(_)
    )
}

/// The focus that `screen` keeps when it opens again as the parent of `left`: the button that
/// opened `left` (WID-SCR-003).
fn kept_focus(left: Screen, screen: Screen) -> Option<WidgetId> {
    match (left, screen) {
        (Screen::VideoSettings, Screen::Options) => Some(options_screen::VIDEO),
        (Screen::AccessibilitySettings, Screen::Options) => Some(options_screen::ACCESSIBILITY),
        (Screen::Controls(_), Screen::Options) => Some(options_screen::CONTROLS),
        (Screen::Controls(_), Screen::AccessibilitySettings) => {
            Some(accessibility_settings_screen::CONTROLS)
        }
        (Screen::MouseSettings(_), Screen::Controls(_)) => Some(controls_screen::MOUSE_SETTINGS),
        _ => None,
    }
}

/// The screen that an activation of `widget` on `shown` opens.
fn target(shown: Screen, widget: WidgetId) -> Option<Screen> {
    match (shown, widget) {
        (Screen::Pause, pause_screen::OPTIONS) => Some(Screen::Options),
        (Screen::Options, options_screen::VIDEO) => Some(Screen::VideoSettings),
        (Screen::Options, options_screen::ACCESSIBILITY) => Some(Screen::AccessibilitySettings),
        (Screen::Options, options_screen::CONTROLS) => {
            Some(Screen::Controls(ControlsParent::Options))
        }
        (Screen::AccessibilitySettings, accessibility_settings_screen::CONTROLS) => {
            Some(Screen::Controls(ControlsParent::AccessibilitySettings))
        }
        (Screen::Controls(parent), controls_screen::MOUSE_SETTINGS) => {
            Some(Screen::MouseSettings(parent))
        }
        (Screen::Pause, pause_screen::BACK_TO_GAME)
        | (Screen::Options, options_screen::DONE)
        | (
            Screen::VideoSettings
            | Screen::AccessibilitySettings
            | Screen::Controls(_)
            | Screen::MouseSettings(_),
            option_sub_screen::DONE,
        ) => Some(parent(shown)),
        _ => None,
    }
}

/// The screen that `event` on `shown` opens.
fn next_screen(shown: Screen, event: MenuEvent) -> Option<Screen> {
    match event {
        MenuEvent::CloseRequested => Some(parent(shown)),
        MenuEvent::Widget(WidgetEvent::Activated { widget, .. }) => target(shown, widget),
        MenuEvent::Widget(_) => None,
    }
}

/// Escape in the game opens the pause screen and frees the mouse pointer (MENU-PAUSE-001).
fn open_pause_screen(
    mut screen: ResMut<Screen>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if *screen == Screen::None {
        *screen = Screen::Pause;
        grab(&mut cursor, false);
    }
}

#[derive(SystemParam)]
struct MenuSource<'w, 's> {
    screen: Res<'w, Screen>,
    scene: Res<'w, GuiScene>,
    options: Res<'w, ClientOptions>,
    language: Res<'w, ClientLanguage>,
    atlas: Res<'w, GuiAtlas>,
    last_key: Res<'w, LastButtonKey>,
    time: Res<'w, Time<Real>>,
    window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
}

/// The scale of Auto for the window pixels of this moment (MENU-SCALE-002, step 1).
fn max_gui_scale(window: &Window) -> u32 {
    GuiSize::auto_scale(window_framebuffer(window))
}

/// The menu of `screen`, or none when it is no menu screen.
fn screen_menu(
    screen: Screen,
    extent: GuiExtent,
    input: GuiInput,
    source: &MenuSource,
) -> Option<Menu> {
    let language = &*source.language;
    let font = &source.atlas.0.font;
    let options = &*source.options;
    let (widgets, tooltips) = match screen {
        Screen::Pause => (
            widgets_of(
                extent,
                input,
                pause_screen(extent, language, font, source.last_key.0)?,
            )?,
            Vec::new(),
        ),
        Screen::Options => (
            widgets_of(
                extent,
                input,
                options_screen(extent, language, font, options.fov)?,
            )?,
            Vec::new(),
        ),
        Screen::VideoSettings => (
            widgets_of(
                extent,
                input,
                video_settings_screen(
                    extent,
                    language,
                    font,
                    options,
                    max_gui_scale(&source.window),
                )?,
            )?,
            Vec::new(),
        ),
        Screen::AccessibilitySettings => (
            widgets_of(
                extent,
                input,
                accessibility_settings_screen(extent, language, font, options)?,
            )?,
            vec![fov_effects_tooltip(language)],
        ),
        Screen::Controls(_) => (
            widgets_of(extent, input, controls_screen(extent, language, font)?)?,
            Vec::new(),
        ),
        Screen::MouseSettings(_) => (
            widgets_of(
                extent,
                input,
                mouse_settings_screen(extent, language, font, options)?,
            )?,
            Vec::new(),
        ),
        Screen::None | Screen::Inventory | Screen::GameModeSwitcher(_) | Screen::Chat => {
            return None;
        }
    };
    let mut menu = Menu::new(widgets, MenuBackground::Texture);
    menu.tooltips = tooltips;
    Some(menu)
}

fn widgets_of(
    extent: GuiExtent,
    input: GuiInput,
    list: impl IntoIterator<Item = Widget>,
) -> Option<Widgets> {
    let mut widgets = Widgets::new(extent, input);
    for widget in list {
        widgets.push(widget).ok()?;
    }
    Some(widgets)
}

/// Inserts, builds again, re-arranges or removes the [`Menu`] of the open screen (WID-SCR-001,
/// WID-SCR-003).
fn project_menu(
    source: MenuSource,
    menu: Option<ResMut<Menu>>,
    mut commands: Commands,
    mut shown: Local<Screen>,
    mut events: Local<Vec<WidgetEvent>>,
) {
    let screen = *source.screen;
    let left = std::mem::replace(&mut *shown, screen);
    let extent = GuiExtent::new(source.scene.size.width, source.scene.size.height);
    let Some(mut menu) = menu else {
        let cursor = source.scene.cursor;
        let input = GuiInput::new(GuiPoint::new(cursor.x, cursor.y));
        if let Some(mut built) = screen_menu(screen, extent, input, &source) {
            built
                .widgets
                .focus_initial(None, menu_clock(&source.time), &mut events);
            commands.insert_resource(built);
        }
        events.clear();
        return;
    };
    let reopened = left != screen;
    let rearranged = menu.widgets.screen() != extent
        || source.options.is_changed()
        || source.language.is_changed();
    if !reopened && !rearranged {
        return;
    }
    let Some(mut built) = screen_menu(screen, extent, menu.widgets.input(), &source) else {
        commands.remove_resource::<Menu>();
        return;
    };
    if reopened || !keeps_widgets(screen) {
        built.widgets.focus_initial(
            kept_focus(left, screen),
            menu_clock(&source.time),
            &mut events,
        );
        events.clear();
        *menu = built;
        return;
    }
    menu.widgets.set_screen(extent);
    for placed in built.widgets.iter() {
        menu.widgets.set_rect(placed.id(), placed.rect());
        if let Some(widget) = menu.widgets.get_mut(placed.id()) {
            // A slider that waits shows its own value, not the stored one (WID-SLD-007, step 3).
            if screen == Screen::VideoSettings
                && let Some(caption) =
                    video_settings_screen::pending_caption(&source.language, widget)
            {
                widget.caption = caption;
            } else {
                widget.caption.clone_from(&placed.caption);
            }
        }
    }
    menu.tooltips = built.tooltips;
}

/// The Ctrl wheel on "Video Settings" steps the GUI scale and never reaches the list or a widget
/// (MENU-SCALE-003).
fn ctrl_wheel_gui_scale(
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll: ResMut<AccumulatedMouseScroll>,
    mut options: ResMut<ClientOptions>,
    menu: Option<ResMut<Menu>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    if !modifiers(&keys).control || scroll.delta.y == 0.0 {
        return;
    }
    let y = std::mem::take(&mut scroll.delta.y);
    if let Some(scale) = wheel_gui_scale(options.gui_scale, max_gui_scale(&window), y) {
        options.gui_scale = scale;
        if let Some(mut menu) = menu {
            menu.widgets.set_scroll_offset(option_sub_screen::LIST, 0.0);
        }
    }
}

/// The value changes of the widgets of the shown screen: each one writes its field of the
/// [`ClientOptions`], and the menu follows. An event that leaves the screen first stores the
/// value of each slider that waits (MENU-SUB-002, step 1).
fn write_menu_options(
    mut events: MessageReader<MenuEvent>,
    screen: Res<Screen>,
    mut menu: ResMut<Menu>,
    mut options: ResMut<ClientOptions>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut changes: Local<Vec<WidgetEvent>>,
) {
    for event in events.read() {
        match *event {
            MenuEvent::Widget(change @ WidgetEvent::ValueChanged { .. }) => changes.push(change),
            // No wait lasts past the end of the clock.
            event if next_screen(*screen, event).is_some() => {
                menu.widgets.apply_deferred(u64::MAX, &mut changes);
            }
            _ => {}
        }
    }
    let mut next = *options;
    for change in changes.drain(..) {
        let WidgetEvent::ValueChanged { widget } = change else {
            continue;
        };
        let Some(widget) = menu.widgets.get(widget) else {
            continue;
        };
        match *screen {
            Screen::Options => options_screen::write_option(&mut next, widget),
            Screen::VideoSettings => video_settings_screen::write_option(
                &mut next,
                widget,
                max_gui_scale(&window),
                modifiers(&keys),
            ),
            Screen::AccessibilitySettings => {
                accessibility_settings_screen::write_option(&mut next, widget);
            }
            Screen::MouseSettings(_) => mouse_settings_screen::write_option(&mut next, widget),
            Screen::None
            | Screen::Inventory
            | Screen::GameModeSwitcher(_)
            | Screen::Chat
            | Screen::Pause
            | Screen::Controls(_) => {}
        }
    }
    options.set_if_neq(next);
}

/// A slider that waits shows its new value at once (WID-SLD-007, step 3).
fn show_pending_values(
    mut events: MessageReader<MenuEvent>,
    screen: Res<Screen>,
    language: Res<ClientLanguage>,
    mut menu: ResMut<Menu>,
) {
    for event in events.read() {
        if let MenuEvent::Widget(WidgetEvent::ValuePending { widget }) = *event
            && *screen == Screen::VideoSettings
            && let Some(widget) = menu.widgets.get_mut(widget)
            && let Some(caption) = video_settings_screen::pending_caption(&language, widget)
        {
            widget.caption = caption;
        }
    }
}

/// The actions of the widgets and of Escape: each one writes the [`Screen`], and the menu
/// follows.
fn follow_menu_events(
    mut events: MessageReader<MenuEvent>,
    mut screen: ResMut<Screen>,
    mut menu: ResMut<Menu>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    let shown = *screen;
    for event in events.read() {
        if *screen != shown {
            continue;
        }
        if let MenuEvent::Widget(WidgetEvent::Activated { widget, .. }) = *event
            && (shown, widget) == (Screen::Pause, pause_screen::LAST)
        {
            if let Some(last) = menu.widgets.get_mut(widget) {
                last.active = false;
            }
            exit.write(AppExit::Success);
        }
        if let Some(next) = next_screen(shown, *event) {
            *screen = next;
            if next == Screen::None {
                grab(&mut cursor, true);
                // The press that closed the screen is no attack in the game.
                buttons.reset_all();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    mod jar {
        use std::sync::{Arc, LazyLock};
        use std::time::Duration;

        use bevy::input::keyboard::{Key, KeyboardInput};
        use bevy::input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel};
        use bevy::input::touch::TouchPhase;
        use bevy::input::{ButtonState, InputPlugin};
        use bevy::time::Real;
        use bevy::window::CursorGrabMode;
        use mcrs_minecraft_client_widget::GuiRect;
        use mcrs_minecraft_model::resource_set::ResourceSet;

        use super::super::*;
        use crate::font::corpus_font;
        use crate::menu_plugin::MenuPlugin;
        use crate::scene::{GuiAtlasData, GuiConfig, GuiSize};

        static ATLAS: LazyLock<Arc<GuiAtlasData>> =
            LazyLock::new(|| Arc::new(GuiAtlasData::load(ResourceSet::corpus()).unwrap()));

        const GUI: GuiExtent = GuiExtent::new(427, 240);

        fn game_app() -> App {
            let mut app = App::new();
            app.add_plugins((
                InputPlugin,
                MenuPlugin,
                MenuScreensPlugin { local_world: true },
            ))
            .insert_resource(GuiConfig {
                scale: 1,
                cursor: Some(IVec2::ZERO),
                frozen_ticks: None,
                advanced_tooltips: false,
            })
            .insert_resource(GuiScene {
                size: GuiSize {
                    width: GUI.width,
                    height: GUI.height,
                    scale: 1,
                },
                ..GuiScene::default()
            })
            .insert_resource(GuiAtlas(Arc::clone(&ATLAS)))
            .insert_resource(ClientLanguage::load(ResourceSet::corpus()).unwrap())
            .init_resource::<ClientOptions>()
            .init_resource::<Screen>()
            .init_resource::<Time<Real>>();
            app.world_mut().spawn((Window::default(), PrimaryWindow));
            app.update();
            app
        }

        fn tap_escape(app: &mut App) {
            for state in [ButtonState::Pressed, ButtonState::Released] {
                app.world_mut().write_message(KeyboardInput {
                    key_code: KeyCode::Escape,
                    logical_key: Key::Escape,
                    state,
                    text: None,
                    repeat: false,
                    window: Entity::PLACEHOLDER,
                });
                app.update();
            }
        }

        fn click(app: &mut App, x: i32, y: i32) {
            app.world_mut().resource_mut::<GuiConfig>().cursor = Some(IVec2::new(x, y));
            for state in [ButtonState::Pressed, ButtonState::Released] {
                app.world_mut().write_message(MouseButtonInput {
                    button: MouseButton::Left,
                    state,
                    window: Entity::PLACEHOLDER,
                });
                app.update();
            }
        }

        fn screen(app: &App) -> Screen {
            *app.world().resource::<Screen>()
        }

        fn grab_mode(app: &mut App) -> CursorGrabMode {
            app.world_mut()
                .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
                .single(app.world())
                .unwrap()
                .grab_mode
        }

        fn caption(app: &App, widget: WidgetId) -> String {
            let menu = app.world().resource::<Menu>();
            menu.widgets.get(widget).unwrap().caption.clone()
        }

        #[test]
        fn escape_opens_the_pause_screen_and_back_to_game_takes_the_mouse_again() {
            let mut app = game_app();
            tap_escape(&mut app);
            assert_eq!(screen(&app), Screen::Pause);
            assert_eq!(grab_mode(&mut app), CursorGrabMode::None);
            assert!(app.world().contains_resource::<Menu>());

            click(&mut app, 150, 90);
            assert_eq!(screen(&app), Screen::None);
            assert_eq!(grab_mode(&mut app), CursorGrabMode::Locked);
            assert!(!app.world().contains_resource::<Menu>());
        }

        #[test]
        fn escape_before_the_atlas_loads_shows_the_pause_screen_once_it_loads() {
            let mut app = App::new();
            app.add_plugins((
                InputPlugin,
                MenuPlugin,
                MenuScreensPlugin { local_world: true },
            ))
            .insert_resource(GuiConfig {
                scale: 1,
                cursor: Some(IVec2::ZERO),
                frozen_ticks: None,
                advanced_tooltips: false,
            })
            .insert_resource(GuiScene {
                size: GuiSize {
                    width: GUI.width,
                    height: GUI.height,
                    scale: 1,
                },
                ..GuiScene::default()
            })
            .init_resource::<ClientOptions>()
            .init_resource::<Screen>()
            .init_resource::<Time<Real>>();
            app.world_mut().spawn((Window::default(), PrimaryWindow));
            app.update();
            let atlas = GuiAtlas(Arc::clone(&ATLAS));
            let language = ClientLanguage::load(ResourceSet::corpus()).unwrap();
            tap_escape(&mut app);
            assert_eq!(screen(&app), Screen::Pause);
            assert!(!app.world().contains_resource::<Menu>());

            app.insert_resource(atlas).insert_resource(language);
            app.update();
            assert!(app.world().contains_resource::<Menu>());
        }

        #[test]
        fn options_then_done_walks_from_the_pause_screen_and_back() {
            let mut app = game_app();
            tap_escape(&mut app);
            click(&mut app, 150, 110);
            assert_eq!(screen(&app), Screen::Options);
            click(&mut app, 200, 220);
            assert_eq!(screen(&app), Screen::Pause);
        }

        #[test]
        fn the_fov_slider_writes_the_options_and_its_caption_follows() {
            let mut app = game_app();
            tap_escape(&mut app);
            click(&mut app, 150, 110);
            assert_eq!(caption(&app, options_screen::FOV), "FOV: Normal");

            click(&mut app, 287, 39);
            assert_eq!(app.world().resource::<ClientOptions>().fov, 110);
            assert_eq!(caption(&app, options_screen::FOV), "FOV: Quake Pro");
        }

        #[test]
        fn the_last_button_ends_the_client() {
            let mut app = game_app();
            tap_escape(&mut app);
            click(&mut app, 150, 135);
            assert_eq!(app.should_exit(), Some(AppExit::Success));
            let menu = app.world().resource::<Menu>();
            assert!(!menu.widgets.get(pause_screen::LAST).unwrap().active);
        }

        #[test]
        fn the_widgets_sit_where_the_screens_place_them() {
            let language = ClientLanguage::corpus();
            let font = corpus_font();
            let pause = pause_screen(GUI, language, font, "menu.returnToMenu").unwrap();
            let options = options_screen(GUI, language, font, 70).unwrap();
            let defaults = ClientOptions::default();
            let video = video_settings_screen(GUI, language, font, &defaults, 2).unwrap();
            let accessibility =
                accessibility_settings_screen(GUI, language, font, &defaults).unwrap();
            let controls = controls_screen(GUI, language, font).unwrap();
            let mouse_settings = mouse_settings_screen(GUI, language, font, &defaults).unwrap();
            let placed: Vec<(String, GuiRect)> = pause
                .iter()
                .chain(&options[1..])
                .chain(&video[1..])
                .chain(&accessibility[1..])
                .chain(&controls)
                .chain(&mouse_settings)
                .map(|widget| (widget.caption.clone(), widget.rect()))
                .collect();
            let expected = [
                ("Game Menu", GuiRect::new(187, 40, 52, 9)),
                ("Back to Game", GuiRect::new(111, 80, 204, 20)),
                ("Options...", GuiRect::new(111, 104, 98, 20)),
                ("Save and Quit to Title", GuiRect::new(111, 128, 204, 20)),
                ("FOV: Normal", GuiRect::new(138, 29, 150, 20)),
                ("Video Settings...", GuiRect::new(59, 91, 150, 20)),
                ("Controls...", GuiRect::new(217, 91, 150, 20)),
                ("Accessibility Settings...", GuiRect::new(59, 115, 150, 20)),
                ("Done", GuiRect::new(113, 214, 200, 20)),
                ("", GuiRect::new(0, 33, 427, 174)),
                ("Display", GuiRect::new(192, 37, 43, 17)),
                ("Max Framerate: 120 FPS", GuiRect::new(58, 55, 150, 20)),
                ("VSync: OFF", GuiRect::new(218, 55, 150, 20)),
                ("GUI Scale: Auto", GuiRect::new(58, 80, 150, 20)),
                ("Fullscreen: OFF", GuiRect::new(218, 80, 150, 20)),
                ("Brightness: Default", GuiRect::new(58, 105, 150, 20)),
                ("Quality & Performance", GuiRect::new(153, 148, 120, 17)),
                ("Render Distance: 96 Chunks", GuiRect::new(58, 166, 150, 20)),
                ("Smooth Lighting: ON", GuiRect::new(58, 191, 150, 20)),
                ("Clouds: Fancy", GuiRect::new(218, 191, 150, 20)),
                ("Done", GuiRect::new(113, 214, 200, 20)),
                ("", GuiRect::new(0, 33, 427, 174)),
                ("Controls...", GuiRect::new(58, 37, 150, 20)),
                ("FOV Effects: 100%", GuiRect::new(58, 62, 150, 20)),
                ("Done", GuiRect::new(138, 214, 150, 20)),
                ("Controls", GuiRect::new(192, 12, 43, 9)),
                ("", GuiRect::new(0, 33, 427, 174)),
                ("Mouse Settings...", GuiRect::new(58, 37, 150, 20)),
                ("Done", GuiRect::new(113, 214, 200, 20)),
                ("Mouse Settings", GuiRect::new(176, 12, 74, 9)),
                ("", GuiRect::new(0, 33, 427, 174)),
                ("Sensitivity: 100%", GuiRect::new(58, 37, 150, 20)),
                ("Done", GuiRect::new(113, 214, 200, 20)),
            ]
            .map(|(caption, rect)| (caption.to_owned(), rect));
            assert_eq!(placed, expected);
            let video_thumb = video[1].scrollbar().map(|scrollbar| scrollbar.thumb);
            assert_eq!(video_thumb, Some(GuiRect::new(376, 33, 6, 165)));
        }

        const VIDEO_BUTTON: (i32, i32) = (100, 100);
        const CONTROLS_BUTTON: (i32, i32) = (250, 100);
        const ACCESSIBILITY_BUTTON: (i32, i32) = (100, 120);
        const FIRST_ROW_BUTTON: (i32, i32) = (100, 40);
        const DONE_BUTTON: (i32, i32) = (200, 220);

        /// Opens the options screen from the game, then clicks each button of `path`.
        fn open_sub_screen(app: &mut App, path: &[(i32, i32)]) {
            tap_escape(app);
            click(app, 150, 110);
            for &(x, y) in path {
                click(app, x, y);
            }
        }

        fn hold_key(app: &mut App, key_code: KeyCode, logical_key: Key) {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }

        fn ctrl_wheel(app: &mut App, y: f32) {
            hold_key(app, KeyCode::ControlLeft, Key::Control);
            wheel(app, y);
        }

        fn wheel(app: &mut App, y: f32) {
            app.world_mut().write_message(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y,
                window: Entity::PLACEHOLDER,
                phase: TouchPhase::Moved,
            });
            app.update();
        }

        fn resize_window(app: &mut App, width: u32, height: u32) {
            app.world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(app.world_mut())
                .unwrap()
                .resolution
                .set_physical_resolution(width, height);
        }

        fn gui_scale(app: &App) -> (u32, String) {
            (
                app.world().resource::<ClientOptions>().gui_scale,
                caption(app, video_settings_screen::GUI_SCALE),
            )
        }

        const VIDEO: &[(i32, i32)] = &[VIDEO_BUTTON];
        const ACCESSIBILITY: &[(i32, i32)] = &[ACCESSIBILITY_BUTTON];
        const MOUSE_SETTINGS: &[(i32, i32)] = &[CONTROLS_BUTTON, FIRST_ROW_BUTTON];

        /// The path to a sub-screen, the clicks on it, the change of the options they make, and
        /// the widget with its caption after them.
        type OptionChange = (
            &'static [(i32, i32)],
            &'static [(i32, i32)],
            fn(&mut ClientOptions),
            WidgetId,
            &'static str,
        );

        const OPTION_CHANGES: [OptionChange; 15] = [
            (
                VIDEO,
                &[(58, 60)],
                |o| o.max_framerate = 10,
                video_settings_screen::MAX_FRAMERATE,
                "Max Framerate: 10 FPS",
            ),
            (
                VIDEO,
                &[(207, 60)],
                |o| o.max_framerate = 260,
                video_settings_screen::MAX_FRAMERATE,
                "Max Framerate: Unlimited",
            ),
            (
                VIDEO,
                &[(260, 60)],
                |o| o.vsync = true,
                video_settings_screen::VSYNC,
                "VSync: ON",
            ),
            (
                VIDEO,
                &[(260, 85)],
                |o| o.fullscreen = true,
                video_settings_screen::FULLSCREEN,
                "Fullscreen: ON",
            ),
            (
                VIDEO,
                &[(58, 110)],
                |o| o.brightness = 0.0,
                video_settings_screen::BRIGHTNESS,
                "Brightness: Moody",
            ),
            (
                VIDEO,
                &[(207, 110)],
                |o| o.brightness = 1.0,
                video_settings_screen::BRIGHTNESS,
                "Brightness: Bright",
            ),
            (
                VIDEO,
                &[(100, 85)],
                |o| o.gui_scale = 1,
                video_settings_screen::GUI_SCALE,
                "GUI Scale: 1",
            ),
            (
                VIDEO,
                &[(100, 195)],
                |o| o.smooth_light = false,
                video_settings_screen::SMOOTH_LIGHTING,
                "Smooth Lighting: OFF",
            ),
            (
                VIDEO,
                &[(260, 195)],
                |o| o.clouds = false,
                video_settings_screen::CLOUDS,
                "Clouds: OFF",
            ),
            (
                ACCESSIBILITY,
                &[(58, 65)],
                |o| o.fov_effect_scale = 0.0,
                accessibility_settings_screen::FOV_EFFECTS,
                "FOV Effects: OFF",
            ),
            (
                ACCESSIBILITY,
                &[(133, 65)],
                |o| o.fov_effect_scale = 0.25,
                accessibility_settings_screen::FOV_EFFECTS,
                "FOV Effects: 25%",
            ),
            (
                ACCESSIBILITY,
                &[(58, 65), (207, 65)],
                |_| {},
                accessibility_settings_screen::FOV_EFFECTS,
                "FOV Effects: 100%",
            ),
            (
                MOUSE_SETTINGS,
                &[(58, 40)],
                |o| o.sensitivity = 0.0,
                mouse_settings_screen::SENSITIVITY,
                "Sensitivity: *yawn*",
            ),
            (
                MOUSE_SETTINGS,
                &[(207, 40)],
                |o| o.sensitivity = 1.0,
                mouse_settings_screen::SENSITIVITY,
                "Sensitivity: HYPERSPEED!!!",
            ),
            (
                MOUSE_SETTINGS,
                &[(63, 40)],
                |o| o.sensitivity = (1.0 / 142.0_f64) as f32,
                mouse_settings_screen::SENSITIVITY,
                "Sensitivity: 1%",
            ),
        ];

        #[test]
        fn each_option_widget_writes_its_option_and_shows_the_new_value() {
            for (sub_screen, clicks, change, widget, shown) in OPTION_CHANGES {
                let mut app = game_app();
                open_sub_screen(&mut app, sub_screen);
                for &(x, y) in clicks {
                    click(&mut app, x, y);
                }
                let mut expected = ClientOptions::default();
                change(&mut expected);
                let options = *app.world().resource::<ClientOptions>();
                assert_eq!((options, caption(&app, widget).as_str()), (expected, shown));
            }
        }

        const RENDER_DISTANCE_LEFT: (i32, i32) = (58, 171);
        const RENDER_DISTANCE_RIGHT: (i32, i32) = (207, 171);

        #[test]
        fn render_distance_stores_its_value_in_the_first_frame_600_ms_after_the_last_change() {
            let cases: [(&[(i32, i32)], &str, Option<u8>); 2] = [
                (
                    &[RENDER_DISTANCE_LEFT],
                    "Render Distance: 2 Chunks",
                    Some(2),
                ),
                (
                    &[RENDER_DISTANCE_LEFT, RENDER_DISTANCE_RIGHT],
                    "Render Distance: 96 Chunks",
                    None,
                ),
            ];
            for (clicks, shown, stored) in cases {
                let mut app = game_app();
                open_sub_screen(&mut app, VIDEO);
                let opened = app.world().resource_ref::<ClientOptions>().last_changed();
                for &(x, y) in clicks {
                    click(&mut app, x, y);
                }
                let pending = caption(&app, video_settings_screen::RENDER_DISTANCE);
                let frames = [599, 600].map(|ms| {
                    let mut time = app.world_mut().resource_mut::<Time<Real>>();
                    time.advance_to(Duration::from_millis(ms));
                    app.update();
                    let options = app.world().resource_ref::<ClientOptions>();
                    (options.view_distance, options.last_changed() != opened)
                });
                assert_eq!(
                    (pending.as_str(), frames),
                    (shown, [(None, false), (stored, stored.is_some())])
                );
            }
        }

        #[test]
        fn a_change_of_another_option_keeps_the_caption_of_the_render_distance_that_waits() {
            let mut app = game_app();
            open_sub_screen(&mut app, VIDEO);
            click(&mut app, RENDER_DISTANCE_LEFT.0, RENDER_DISTANCE_LEFT.1);
            app.world_mut().resource_mut::<ClientOptions>().vsync = true;
            app.update();
            let waiting = caption(&app, video_settings_screen::RENDER_DISTANCE);
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_to(Duration::from_millis(600));
            app.update();
            app.update();
            let stored = app.world().resource::<ClientOptions>().view_distance;
            let shown = caption(&app, video_settings_screen::RENDER_DISTANCE);
            assert_eq!(
                (waiting.as_str(), stored, shown.as_str()),
                (
                    "Render Distance: 2 Chunks",
                    Some(2),
                    "Render Distance: 2 Chunks"
                )
            );
        }

        #[test]
        fn escape_or_done_stores_the_render_distance_that_waits_at_once() {
            let done: fn(&mut App) = |app| click(app, DONE_BUTTON.0, DONE_BUTTON.1);
            for close in [tap_escape, done] {
                let mut app = game_app();
                open_sub_screen(&mut app, VIDEO);
                click(&mut app, RENDER_DISTANCE_LEFT.0, RENDER_DISTANCE_LEFT.1);
                close(&mut app);
                let stored = app.world().resource::<ClientOptions>().view_distance;
                assert_eq!((screen(&app), stored), (Screen::Options, Some(2)));
            }
        }

        #[test]
        fn the_gui_scale_values_follow_the_window_of_each_change() {
            let mut app = game_app();
            open_sub_screen(&mut app, &[VIDEO_BUTTON]);
            resize_window(&mut app, 854, 480);
            let activations = [(); 3].map(|()| {
                click(&mut app, 100, 85);
                gui_scale(&app)
            });
            assert_eq!(
                activations,
                [
                    (1, "GUI Scale: 1"),
                    (2, "GUI Scale: 2"),
                    (0, "GUI Scale: Auto")
                ]
                .map(|(value, text)| (value, text.to_owned()))
            );

            resize_window(&mut app, 1920, 1080);
            app.world_mut().resource_mut::<GuiScene>().size = GuiSize {
                width: GUI.width,
                height: 150,
                scale: 1,
            };
            let list_offset = |app: &App| {
                let menu = app.world().resource::<Menu>();
                menu.widgets
                    .get(option_sub_screen::LIST)
                    .and_then(Widget::scroll_offset)
            };
            wheel(&mut app, -1.0);
            assert!(list_offset(&app).is_some_and(|offset| offset > 0.0));
            ctrl_wheel(&mut app, -1.0);
            assert_eq!(list_offset(&app), Some(0.0));
            ctrl_wheel(&mut app, 1.0);
            assert_eq!(gui_scale(&app), (4, "GUI Scale: 4".to_owned()));
            hold_key(&mut app, KeyCode::ShiftLeft, Key::Shift);
            click(&mut app, 100, 85);
            assert_eq!(gui_scale(&app), (3, "GUI Scale: 3".to_owned()));
        }

        #[test]
        fn a_new_gui_size_moves_the_widgets_and_keeps_the_focus() {
            let mut app = game_app();
            open_sub_screen(&mut app, &[VIDEO_BUTTON]);
            click(&mut app, 100, 85);
            app.world_mut().resource_mut::<GuiScene>().size = GuiSize {
                width: 854,
                height: 480,
                scale: 1,
            };
            app.update();
            let menu = app.world().resource::<Menu>();
            let gui_scale = menu.widgets.get(video_settings_screen::GUI_SCALE).unwrap();
            assert_eq!(
                (
                    menu.widgets.focused(),
                    gui_scale.rect(),
                    gui_scale.caption.as_str()
                ),
                (
                    Some(video_settings_screen::GUI_SCALE),
                    GuiRect::new(272, 80, 150, 20),
                    "GUI Scale: 1"
                )
            );
        }

        #[test]
        fn controls_and_mouse_settings_close_to_the_screen_that_opened_them() {
            type Step = fn(&mut App);
            let open_mouse_settings: Step =
                |app| click(app, FIRST_ROW_BUTTON.0, FIRST_ROW_BUTTON.1);
            let done: Step = |app| click(app, DONE_BUTTON.0, DONE_BUTTON.1);
            let cases: [(&[(i32, i32)], _, _, _, Step); 2] = [
                (
                    &[CONTROLS_BUTTON],
                    ControlsParent::Options,
                    Screen::Options,
                    options_screen::CONTROLS,
                    tap_escape,
                ),
                (
                    &[ACCESSIBILITY_BUTTON, FIRST_ROW_BUTTON],
                    ControlsParent::AccessibilitySettings,
                    Screen::AccessibilitySettings,
                    accessibility_settings_screen::CONTROLS,
                    done,
                ),
            ];
            for (path, parent, opener, opener_button, close_controls) in cases {
                let mut app = game_app();
                open_sub_screen(&mut app, path);
                let shown = |app: &App| {
                    let menu = app.world().resource::<Menu>();
                    (screen(app), menu.widgets.focused())
                };
                let mut seen = vec![shown(&app)];
                for step in [
                    open_mouse_settings,
                    tap_escape,
                    open_mouse_settings,
                    done,
                    close_controls,
                ] {
                    step(&mut app);
                    seen.push(shown(&app));
                }
                let mouse_settings = Some(controls_screen::MOUSE_SETTINGS);
                assert_eq!(
                    seen,
                    [
                        (Screen::Controls(parent), None),
                        (Screen::MouseSettings(parent), None),
                        (Screen::Controls(parent), mouse_settings),
                        (Screen::MouseSettings(parent), None),
                        (Screen::Controls(parent), mouse_settings),
                        (opener, Some(opener_button)),
                    ]
                );
            }
        }

        #[test]
        fn the_fullscreen_button_shows_a_change_from_the_key_and_inverts_the_stored_value() {
            let mut app = game_app();
            open_sub_screen(&mut app, &[VIDEO_BUTTON]);
            app.world_mut().resource_mut::<ClientOptions>().fullscreen = true;
            app.update();
            let shown = caption(&app, video_settings_screen::FULLSCREEN);
            click(&mut app, 260, 85);
            assert_eq!(
                (
                    shown.as_str(),
                    app.world().resource::<ClientOptions>().fullscreen,
                    caption(&app, video_settings_screen::FULLSCREEN).as_str()
                ),
                ("Fullscreen: ON", false, "Fullscreen: OFF")
            );
        }
    }
}
