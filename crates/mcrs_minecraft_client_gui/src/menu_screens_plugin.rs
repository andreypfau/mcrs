use bevy::input::InputSystems;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::{WidgetEvent, WidgetId};
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::{
    AccessibilityParent, ControlsParent, CreateWorldParent, Screen,
};
use mcrs_minecraft_client_world::session::{LocalServerStopped, OpenLocalWorld};
use mcrs_minecraft_client_world::world_list::{
    CreateWorld, DeleteWorld, PlayWorld, ReadWorldList, WorldList, WorldStoreFailed, WorldTypes,
};
use mcrs_minecraft_network::client::{ConnectionEnded, LeaveRequest};
use time::UtcOffset;

use crate::accessibility_settings_screen;
use crate::controls_screen;
use crate::create_world_screen::{self, NewWorldForm};
use crate::cursor_grab::grab;
use crate::delete_world_screen;
use crate::disconnected_screen;
use crate::follow_create_world::{follow_create_world, on_create_world};
use crate::follow_list_answer::follow_list_answer;
use crate::follow_menu_events::{DeleteTarget, follow_menu_events};
use crate::language::ClientLanguage;
use crate::menu::Menu;
use crate::menu_draw::WorldPresence;
use crate::menu_event::MenuEvent;
use crate::menu_input::modifiers;
use crate::menu_input_systems::MenuInputSystems;
use crate::menu_options_systems::MenuOptionsSystems;
use crate::mouse_settings_screen;
use crate::option_sub_screen;
use crate::options_screen;
use crate::pause_screen;
use crate::project_menu::{SingleplayerActive, max_gui_scale, project_menu};
use crate::scene::GuiAtlas;
use crate::select_world_screen;
use crate::session_screens::{DisconnectReason, follow_session, show_failed_start};
use crate::title_draw::TitleScreenState;
use crate::title_screen;
use crate::video_settings_screen::{self, wheel_gui_scale};
use crate::world_entry::LocalOffset;
use crate::world_list_error_screen;
use crate::world_rows::{PreselectedWorld, project_world_rows};

pub struct MenuScreensPlugin {
    pub preselected_world: Option<String>,
    pub utc_offset: UtcOffset,
    pub singleplayer: bool,
}

impl Plugin for MenuScreensPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PreselectedWorld(self.preselected_world.clone()))
            .insert_resource(LocalOffset(self.utc_offset))
            .insert_resource(SingleplayerActive(self.singleplayer))
            .init_resource::<DeleteTarget>()
            .init_resource::<WorldList>()
            .init_resource::<WorldTypes>()
            .init_resource::<NewWorldForm>()
            .init_resource::<TitleScreenState>()
            .init_resource::<DisconnectReason>()
            .add_message::<OpenLocalWorld>()
            .add_message::<LocalServerStopped>()
            .add_message::<ConnectionEnded>()
            .add_message::<LeaveRequest>()
            .add_message::<ReadWorldList>()
            .add_message::<PlayWorld>()
            .add_message::<DeleteWorld>()
            .add_message::<CreateWorld>()
            .add_message::<WorldStoreFailed>()
            .add_systems(
                PreUpdate,
                (
                    (
                        open_pause_screen.run_if(
                            input_just_pressed(KeyCode::Escape)
                                .and_then(any_with_component::<Player>),
                        ),
                        ctrl_wheel_gui_scale
                            .run_if(resource_equals(Screen::VideoSettings))
                            .in_set(MenuOptionsSystems),
                        follow_session.run_if(
                            not(resource_equals(Screen::None))
                                .or_else(on_message::<ConnectionEnded>)
                                .or_else(on_message::<LocalServerStopped>),
                        ),
                        project_menu.run_if(
                            resource_exists::<GuiAtlas>
                                .and_then(resource_exists::<ClientLanguage>)
                                .and_then(
                                    resource_changed::<Screen>.or_else(resource_exists::<Menu>),
                                ),
                        ),
                    )
                        .chain()
                        .after(InputSystems)
                        .before(MenuInputSystems),
                    (
                        follow_list_answer.run_if(resource_equals(Screen::SelectWorld)),
                        project_world_rows.run_if(
                            resource_equals(Screen::SelectWorld)
                                .and_then(resource_exists::<GuiAtlas>)
                                .and_then(resource_exists::<ClientLanguage>),
                        ),
                        write_menu_options.in_set(MenuOptionsSystems),
                        show_failed_start.run_if(on_message::<WorldStoreFailed>),
                        follow_create_world.run_if(on_create_world),
                        follow_menu_events,
                    )
                        .chain()
                        .after(MenuInputSystems)
                        .run_if(resource_exists::<Menu>),
                ),
            );
    }
}

fn parent(screen: Screen, world: WorldPresence) -> Screen {
    match screen {
        Screen::Options => match world {
            WorldPresence::InWorld => Screen::Pause,
            WorldPresence::NoWorld => Screen::Title,
        },
        Screen::AccessibilitySettings(AccessibilityParent::Title) => Screen::Title,
        Screen::VideoSettings
        | Screen::AccessibilitySettings(AccessibilityParent::Options)
        | Screen::Controls(ControlsParent::Options) => Screen::Options,
        Screen::Controls(ControlsParent::AccessibilitySettings(parent)) => {
            Screen::AccessibilitySettings(parent)
        }
        Screen::MouseSettings(parent) => Screen::Controls(parent),
        Screen::SelectWorld | Screen::CreateWorld(CreateWorldParent::Title) => Screen::Title,
        Screen::DeleteWorldConfirmation | Screen::CreateWorld(CreateWorldParent::SelectWorld) => {
            Screen::SelectWorld
        }
        Screen::None
        | Screen::Title
        | Screen::Menu
        | Screen::GameModeSwitcher(_)
        | Screen::Chat
        | Screen::Pause
        | Screen::Message(_)
        | Screen::LevelLoading
        | Screen::Disconnected
        | Screen::WorldListError => Screen::None,
    }
}

fn target(shown: Screen, widget: WidgetId, world: WorldPresence) -> Option<Screen> {
    match (shown, widget) {
        (Screen::Pause, pause_screen::OPTIONS) | (Screen::Title, title_screen::OPTIONS) => {
            Some(Screen::Options)
        }
        (Screen::Title, title_screen::ACCESSIBILITY) => {
            Some(Screen::AccessibilitySettings(AccessibilityParent::Title))
        }
        (Screen::Options, options_screen::VIDEO) => Some(Screen::VideoSettings),
        (Screen::Options, options_screen::ACCESSIBILITY) => {
            Some(Screen::AccessibilitySettings(AccessibilityParent::Options))
        }
        (Screen::Options, options_screen::CONTROLS) => {
            Some(Screen::Controls(ControlsParent::Options))
        }
        (Screen::AccessibilitySettings(parent), accessibility_settings_screen::CONTROLS) => Some(
            Screen::Controls(ControlsParent::AccessibilitySettings(parent)),
        ),
        (Screen::Controls(parent), controls_screen::MOUSE_SETTINGS) => {
            Some(Screen::MouseSettings(parent))
        }
        (Screen::Disconnected, disconnected_screen::BACK)
        | (Screen::WorldListError, world_list_error_screen::CANCEL)
        | (Screen::SelectWorld, select_world_screen::BACK) => Some(Screen::Title),
        (Screen::Title, title_screen::SINGLEPLAYER) => Some(Screen::SelectWorld),
        (Screen::SelectWorld, select_world_screen::DELETE) => Some(Screen::DeleteWorldConfirmation),
        (Screen::SelectWorld, select_world_screen::CREATE) => {
            Some(Screen::CreateWorld(CreateWorldParent::SelectWorld))
        }
        (
            Screen::DeleteWorldConfirmation,
            delete_world_screen::DELETE | delete_world_screen::CANCEL,
        ) => Some(Screen::SelectWorld),
        (Screen::Pause, pause_screen::BACK_TO_GAME)
        | (Screen::Options, options_screen::DONE)
        | (Screen::CreateWorld(_), create_world_screen::CANCEL)
        | (
            Screen::VideoSettings
            | Screen::AccessibilitySettings(_)
            | Screen::Controls(_)
            | Screen::MouseSettings(_),
            option_sub_screen::DONE,
        ) => Some(parent(shown, world)),
        _ => None,
    }
}

pub(crate) fn next_screen(shown: Screen, event: MenuEvent, world: WorldPresence) -> Option<Screen> {
    match event {
        MenuEvent::CloseRequested
            if matches!(
                shown,
                Screen::Title
                    | Screen::Message(_)
                    | Screen::LevelLoading
                    | Screen::Disconnected
                    | Screen::WorldListError
            ) =>
        {
            None
        }
        MenuEvent::CloseRequested => Some(parent(shown, world)),
        MenuEvent::Widget(WidgetEvent::Activated { widget, .. }) => target(shown, widget, world),
        MenuEvent::Widget(_) | MenuEvent::Confirmed => None,
    }
}

pub(crate) fn world_presence(players: &Query<(), With<Player>>) -> WorldPresence {
    if players.is_empty() {
        WorldPresence::NoWorld
    } else {
        WorldPresence::InWorld
    }
}

fn open_pause_screen(
    mut screen: ResMut<Screen>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if *screen == Screen::None {
        *screen = Screen::Pause;
        grab(&mut cursor, false);
    }
}

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

fn write_menu_options(
    mut events: MessageReader<MenuEvent>,
    screen: Res<Screen>,
    mut menu: ResMut<Menu>,
    mut options: ResMut<ClientOptions>,
    keys: Res<ButtonInput<KeyCode>>,
    (window, players): (
        Single<&Window, With<PrimaryWindow>>,
        Query<(), With<Player>>,
    ),
    mut changes: Local<Vec<WidgetEvent>>,
) {
    let world = world_presence(&players);
    for event in events.read() {
        match *event {
            MenuEvent::Widget(change @ WidgetEvent::ValueChanged { .. }) => changes.push(change),
            event if next_screen(*screen, event, world).is_some() => {
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
            Screen::AccessibilitySettings(_) => {
                accessibility_settings_screen::write_option(&mut next, widget);
            }
            Screen::MouseSettings(_) => mouse_settings_screen::write_option(&mut next, widget),
            Screen::None
            | Screen::Title
            | Screen::Menu
            | Screen::GameModeSwitcher(_)
            | Screen::Chat
            | Screen::Pause
            | Screen::Controls(_)
            | Screen::Message(_)
            | Screen::LevelLoading
            | Screen::Disconnected
            | Screen::WorldListError
            | Screen::SelectWorld
            | Screen::DeleteWorldConfirmation
            | Screen::CreateWorld(_) => {}
        }
    }
    options.set_if_neq(next);
}

#[cfg(test)]
mod tests {
    mod jar {
        use std::sync::Arc;

        use bevy::input::keyboard::{Key, KeyboardInput};
        use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
        use bevy::input::touch::TouchPhase;
        use bevy::input::{ButtonState, InputPlugin};
        use bevy::time::Real;
        use bevy::window::CursorGrabMode;
        use mcrs_minecraft_client_widget::{GuiRect, Widget};
        use mcrs_minecraft_client_world::session::{
            LocalServer, LocalServerPhase, LocalWorldOrigin,
        };
        use mcrs_minecraft_model::resource_set::ResourceSet;
        use time::UtcOffset;

        use super::super::*;
        use crate::accessibility_settings_screen::accessibility_settings_screen;
        use crate::controls_screen::controls_screen;
        use crate::font::corpus_font;
        use crate::menu_plugin::MenuPlugin;
        use crate::menu_test_app::{
            ATLAS, GUI, caption, click, game_app, grab_mode, screen, tap_escape,
        };
        use crate::mouse_settings_screen::mouse_settings_screen;
        use crate::options_screen::options_screen;
        use crate::pause_screen::pause_screen;
        use crate::scene::{GuiConfig, GuiScene, GuiSize};
        use crate::video_settings_screen::video_settings_screen;

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
                MenuScreensPlugin {
                    preselected_world: None,
                    utc_offset: UtcOffset::UTC,
                    singleplayer: true,
                },
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
            app.world_mut().spawn(Player);
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
        fn the_last_button_of_a_local_world_leaves_the_session() {
            let mut app = game_app();
            app.insert_resource(LocalServer {
                phase: LocalServerPhase::Running,
                origin: LocalWorldOrigin::Opened,
            });
            tap_escape(&mut app);
            assert_eq!(caption(&app, pause_screen::LAST), "Save and Quit to Title");
            click(&mut app, 150, 135);
            assert_eq!(app.should_exit(), None);
            assert!(!app.world().resource::<Messages<LeaveRequest>>().is_empty());
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
                ("Improved Transparency: OFF", GuiRect::new(58, 216, 150, 20)),
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
            assert_eq!(video_thumb, Some(GuiRect::new(376, 33, 6, 145)));
        }

        const VIDEO_BUTTON: (i32, i32) = (100, 100);
        const CONTROLS_BUTTON: (i32, i32) = (250, 100);
        const ACCESSIBILITY_BUTTON: (i32, i32) = (100, 120);
        const FIRST_ROW_BUTTON: (i32, i32) = (100, 40);
        const DONE_BUTTON: (i32, i32) = (200, 220);

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

        type OptionChange = (
            &'static [(i32, i32)],
            &'static [(i32, i32)],
            fn(&mut ClientOptions),
            WidgetId,
            &'static str,
        );

        const OPTION_CHANGES: [OptionChange; 17] = [
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
                &[(58, 171)],
                |o| o.view_distance = Some(2),
                video_settings_screen::RENDER_DISTANCE,
                "Render Distance: 2 Chunks",
            ),
            (
                VIDEO,
                &[(58, 171), (207, 171)],
                |o| o.view_distance = Some(96),
                video_settings_screen::RENDER_DISTANCE,
                "Render Distance: 96 Chunks",
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

        #[test]
        fn improved_transparency_shows_after_the_list_scrolls_and_a_press_turns_it_on() {
            let mut app = game_app();
            open_sub_screen(&mut app, VIDEO);
            wheel(&mut app, -10.0);
            click(&mut app, 100, 190);
            assert_eq!(
                (
                    app.world()
                        .resource::<ClientOptions>()
                        .improved_transparency,
                    caption(&app, video_settings_screen::IMPROVED_TRANSPARENCY).as_str(),
                ),
                (true, "Improved Transparency: ON")
            );
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
                    ControlsParent::AccessibilitySettings(AccessibilityParent::Options),
                    Screen::AccessibilitySettings(AccessibilityParent::Options),
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
