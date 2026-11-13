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
use mcrs_minecraft_client_world::screen::Screen;

use crate::accessibility_settings_screen::{
    self, accessibility_settings_screen, fov_effects_tooltip,
};
use crate::cursor_grab::grab;
use crate::language::ClientLanguage;
use crate::menu::{Menu, MenuBackground};
use crate::menu_event::MenuEvent;
use crate::menu_input::{menu_clock, modifiers};
use crate::menu_input_systems::MenuInputSystems;
use crate::menu_options_systems::MenuOptionsSystems;
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
        Screen::VideoSettings | Screen::AccessibilitySettings => Screen::Options,
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
        Screen::Options | Screen::VideoSettings | Screen::AccessibilitySettings
    )
}

/// The focus that `screen` keeps when it opens again as the parent of `left`: the button that
/// opened `left` (WID-SCR-003).
fn kept_focus(left: Screen, screen: Screen) -> Option<WidgetId> {
    match (left, screen) {
        (Screen::VideoSettings, Screen::Options) => Some(options_screen::VIDEO),
        (Screen::AccessibilitySettings, Screen::Options) => Some(options_screen::ACCESSIBILITY),
        _ => None,
    }
}

/// The screen that an activation of `widget` on `shown` opens.
fn target(shown: Screen, widget: WidgetId) -> Option<Screen> {
    match (shown, widget) {
        (Screen::Pause, pause_screen::OPTIONS) => Some(Screen::Options),
        (Screen::Options, options_screen::VIDEO) => Some(Screen::VideoSettings),
        (Screen::Options, options_screen::ACCESSIBILITY) => Some(Screen::AccessibilitySettings),
        (Screen::Pause, pause_screen::BACK_TO_GAME)
        | (Screen::Options, options_screen::DONE)
        | (Screen::VideoSettings | Screen::AccessibilitySettings, option_sub_screen::DONE) => {
            Some(parent(shown))
        }
        _ => None,
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
            widget.caption.clone_from(&placed.caption);
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
/// [`ClientOptions`], and the menu follows.
fn write_menu_options(
    mut events: MessageReader<MenuEvent>,
    screen: Res<Screen>,
    menu: Res<Menu>,
    mut options: ResMut<ClientOptions>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let mut next = *options;
    for event in events.read() {
        let MenuEvent::Widget(WidgetEvent::ValueChanged { widget }) = *event else {
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
            Screen::None
            | Screen::Inventory
            | Screen::GameModeSwitcher(_)
            | Screen::Chat
            | Screen::Pause => {}
        }
    }
    options.set_if_neq(next);
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
        let next = match *event {
            MenuEvent::CloseRequested => Some(parent(shown)),
            MenuEvent::Widget(WidgetEvent::Activated { widget, .. }) => {
                if (shown, widget) == (Screen::Pause, pause_screen::LAST) {
                    if let Some(last) = menu.widgets.get_mut(widget) {
                        last.active = false;
                    }
                    exit.write(AppExit::Success);
                }
                target(shown, widget)
            }
            MenuEvent::Widget(_) => None,
        };
        if let Some(next) = next {
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
            let placed: Vec<(String, GuiRect)> = pause
                .iter()
                .chain(&options[1..])
                .chain(&video[1..])
                .chain(&accessibility[1..])
                .map(|widget| (widget.caption.clone(), widget.rect()))
                .collect();
            let expected = [
                ("Game Menu", GuiRect::new(187, 40, 52, 9)),
                ("Back to Game", GuiRect::new(111, 80, 204, 20)),
                ("Options...", GuiRect::new(111, 104, 98, 20)),
                ("Save and Quit to Title", GuiRect::new(111, 128, 204, 20)),
                ("FOV: Normal", GuiRect::new(138, 29, 150, 20)),
                ("Video Settings...", GuiRect::new(59, 91, 150, 20)),
                ("Accessibility Settings...", GuiRect::new(217, 91, 150, 20)),
                ("Done", GuiRect::new(113, 214, 200, 20)),
                ("", GuiRect::new(0, 33, 427, 174)),
                ("Display", GuiRect::new(192, 37, 43, 17)),
                ("GUI Scale: Auto", GuiRect::new(58, 55, 150, 20)),
                ("Brightness: Default", GuiRect::new(218, 55, 150, 20)),
                ("Quality & Performance", GuiRect::new(153, 98, 120, 17)),
                ("Smooth Lighting: ON", GuiRect::new(58, 116, 150, 20)),
                ("Clouds: Fancy", GuiRect::new(218, 116, 150, 20)),
                ("Done", GuiRect::new(113, 214, 200, 20)),
                ("", GuiRect::new(0, 33, 427, 174)),
                ("FOV Effects: 100%", GuiRect::new(58, 37, 150, 20)),
                ("Done", GuiRect::new(138, 214, 150, 20)),
            ]
            .map(|(caption, rect)| (caption.to_owned(), rect));
            assert_eq!(placed, expected);
        }

        const VIDEO_BUTTON: (i32, i32) = (100, 100);
        const ACCESSIBILITY_BUTTON: (i32, i32) = (250, 100);

        fn open_sub_screen(app: &mut App, (x, y): (i32, i32)) {
            tap_escape(app);
            click(app, 150, 110);
            click(app, x, y);
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

        #[test]
        fn each_option_widget_writes_its_option_and_shows_the_new_value() {
            type Change = fn(&mut ClientOptions);
            let cases: [(_, &[(i32, i32)], Change, _, &str); 8] = [
                (
                    VIDEO_BUTTON,
                    &[(218, 60)],
                    |o| o.brightness = 0.0,
                    video_settings_screen::BRIGHTNESS,
                    "Brightness: Moody",
                ),
                (
                    VIDEO_BUTTON,
                    &[(367, 60)],
                    |o| o.brightness = 1.0,
                    video_settings_screen::BRIGHTNESS,
                    "Brightness: Bright",
                ),
                (
                    VIDEO_BUTTON,
                    &[(100, 60)],
                    |o| o.gui_scale = 1,
                    video_settings_screen::GUI_SCALE,
                    "GUI Scale: 1",
                ),
                (
                    VIDEO_BUTTON,
                    &[(100, 120)],
                    |o| o.smooth_light = false,
                    video_settings_screen::SMOOTH_LIGHTING,
                    "Smooth Lighting: OFF",
                ),
                (
                    VIDEO_BUTTON,
                    &[(260, 120)],
                    |o| o.clouds = false,
                    video_settings_screen::CLOUDS,
                    "Clouds: OFF",
                ),
                (
                    ACCESSIBILITY_BUTTON,
                    &[(58, 40)],
                    |o| o.fov_effect_scale = 0.0,
                    accessibility_settings_screen::FOV_EFFECTS,
                    "FOV Effects: OFF",
                ),
                (
                    ACCESSIBILITY_BUTTON,
                    &[(133, 40)],
                    |o| o.fov_effect_scale = 0.25,
                    accessibility_settings_screen::FOV_EFFECTS,
                    "FOV Effects: 25%",
                ),
                (
                    ACCESSIBILITY_BUTTON,
                    &[(58, 40), (207, 40)],
                    |_| {},
                    accessibility_settings_screen::FOV_EFFECTS,
                    "FOV Effects: 100%",
                ),
            ];
            for (sub_screen, clicks, change, widget, shown) in cases {
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

        #[test]
        fn the_gui_scale_values_follow_the_window_of_each_change() {
            let mut app = game_app();
            open_sub_screen(&mut app, VIDEO_BUTTON);
            resize_window(&mut app, 854, 480);
            let activations = [(); 3].map(|()| {
                click(&mut app, 100, 60);
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
            click(&mut app, 100, 60);
            assert_eq!(gui_scale(&app), (3, "GUI Scale: 3".to_owned()));
        }

        #[test]
        fn a_new_gui_size_moves_the_widgets_and_keeps_the_focus() {
            let mut app = game_app();
            open_sub_screen(&mut app, VIDEO_BUTTON);
            click(&mut app, 100, 60);
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
                    GuiRect::new(272, 55, 150, 20),
                    "GUI Scale: 1"
                )
            );
        }
    }
}
