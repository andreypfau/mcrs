use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_widget::{WidgetEvent, Widgets};
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::{CreateWorldParent, Screen, ScreenMessage};
use mcrs_minecraft_client_world::world_list::{DeleteWorld, PlayWorld, ReadWorldList, WorldList};
use mcrs_minecraft_network::client::LeaveRequest;

use crate::create_world_screen::NewWorldForm;
use crate::cursor_grab::grab;
use crate::delete_world_screen;
use crate::disconnected_screen;
use crate::menu::Menu;
use crate::menu_event::MenuEvent;
use crate::menu_screens_plugin::{next_screen, world_presence};
use crate::pause_screen;
use crate::select_world_screen::{self, selected_row, selected_world, world_at};
use crate::title_draw::TitleScreenState;
use crate::title_screen;
use crate::world_list_error_screen;
#[derive(Resource, Default, PartialEq)]
pub(crate) struct DeleteTarget(pub(crate) Option<String>);

#[derive(SystemParam)]
pub(crate) struct TitleActions<'w, 's> {
    players: Query<'w, 's, (), With<Player>>,
    state: ResMut<'w, TitleScreenState>,
    worlds: Res<'w, WorldList>,
    delete_target: ResMut<'w, DeleteTarget>,
    form: ResMut<'w, NewWorldForm>,
    time: Res<'w, Time<Real>>,
    reads: MessageWriter<'w, ReadWorldList>,
    plays: MessageWriter<'w, PlayWorld>,
    deletes: MessageWriter<'w, DeleteWorld>,
}

impl TitleActions<'_, '_> {
    fn play(&mut self, widgets: &Widgets, row: Option<usize>, screen: &mut Screen) {
        if let Some(world) = row.and_then(|row| world_at(widgets, &self.worlds, row))
            && !world.is_disabled()
        {
            self.plays.write(PlayWorld {
                folder_id: world.folder_id.clone(),
            });
            *screen = Screen::Message(ScreenMessage::ReadingWorldData);
        }
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "allocates a world folder id only when the player plays or deletes a world"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(4))]
pub(crate) fn follow_menu_events(
    mut events: MessageReader<MenuEvent>,
    mut screen: ResMut<Screen>,
    mut menu: ResMut<Menu>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    (mut exit, mut leave): (MessageWriter<AppExit>, MessageWriter<LeaveRequest>),
    mut title: TitleActions,
) {
    let shown = *screen;
    let world = world_presence(&title.players);
    for event in events.read() {
        if *screen != shown {
            continue;
        }
        if let MenuEvent::Widget(WidgetEvent::Activated { widget, .. }) = *event {
            match (shown, widget) {
                (Screen::Pause, pause_screen::LAST) => {
                    if let Some(last) = menu.widgets.get_mut(widget) {
                        last.active = false;
                    }
                    leave.write(LeaveRequest);
                }
                (Screen::Disconnected, disconnected_screen::BACK)
                | (Screen::WorldListError, world_list_error_screen::CANCEL) => {
                    title.state.end_fade();
                }
                (Screen::Title, title_screen::QUIT) => {
                    exit.write(AppExit::Success);
                }
                (Screen::Title, title_screen::SINGLEPLAYER) => {
                    title.reads.write(ReadWorldList);
                    title.state.end_fade();
                }
                (Screen::SelectWorld, select_world_screen::PLAY) => {
                    title.play(&menu.widgets, selected_row(&menu.widgets), &mut screen);
                }
                (Screen::SelectWorld, select_world_screen::DELETE) => {
                    title.delete_target.set_if_neq(DeleteTarget(
                        selected_world(&menu.widgets, &title.worlds)
                            .map(|world| world.folder_id.clone()),
                    ));
                }
                (Screen::SelectWorld, select_world_screen::CREATE) => {
                    title
                        .form
                        .set_if_neq(NewWorldForm::opened(title.time.elapsed()));
                }
                (Screen::DeleteWorldConfirmation, delete_world_screen::DELETE) => {
                    if let Some(folder_id) = title.delete_target.0.take() {
                        title.deletes.write(DeleteWorld { folder_id });
                    }
                }
                (Screen::DeleteWorldConfirmation, delete_world_screen::CANCEL) => {
                    title.reads.write(ReadWorldList);
                }
                _ => {}
            }
        }
        match (shown, *event) {
            (
                Screen::SelectWorld,
                MenuEvent::Widget(WidgetEvent::RowActivated {
                    widget: select_world_screen::LIST,
                    row,
                }),
            ) => {
                title.play(&menu.widgets, Some(row), &mut screen);
            }
            (Screen::DeleteWorldConfirmation, MenuEvent::CloseRequested) => {
                title.reads.write(ReadWorldList);
            }
            (Screen::CreateWorld(CreateWorldParent::SelectWorld), event)
                if next_screen(shown, event, world) == Some(Screen::SelectWorld) =>
            {
                title.reads.write(ReadWorldList);
            }
            _ => {}
        }
        if let Some(next) = next_screen(shown, *event, world) {
            screen.set_if_neq(next);
            if next == Screen::None {
                grab(&mut cursor, true);
                buttons.reset_all();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    mod jar {
        use bevy::input::ButtonState;
        use bevy::input::keyboard::{Key, KeyboardInput};
        use bevy::window::CursorGrabMode;
        use mcrs_minecraft_client_widget::WidgetKind;
        use mcrs_minecraft_client_world::screen::AccessibilityParent;

        use super::super::*;
        use crate::menu_test_app::{
            advance, click, drained, grab_mode, press, screen, tap_escape, title_app, type_text,
            world,
        };

        #[test]
        fn the_screens_of_the_title_screen_close_to_it_and_escape_leaves_it_open() {
            let mut app = title_app();
            let shown = |app: &App| {
                let menu = app.world().resource::<Menu>();
                (screen(app), menu.widgets.focused())
            };
            type Step = fn(&mut App);
            let steps: [Step; 7] = [
                tap_escape,
                |app| click(app, 120, 165),
                tap_escape,
                |app| click(app, 210, 140),
                |app| click(app, 200, 220),
                |app| click(app, 213, 118),
                tap_escape,
            ];
            let mut seen = vec![shown(&app)];
            for step in steps {
                step(&mut app);
                seen.push(shown(&app));
            }
            assert_eq!(
                seen,
                [
                    (Screen::Title, None),
                    (Screen::Title, None),
                    (Screen::Options, None),
                    (Screen::Title, Some(title_screen::OPTIONS)),
                    (
                        Screen::AccessibilitySettings(AccessibilityParent::Title),
                        None
                    ),
                    (Screen::Title, Some(title_screen::ACCESSIBILITY)),
                    (Screen::SelectWorld, Some(select_world_screen::SEARCH)),
                    (Screen::Title, Some(title_screen::SINGLEPLAYER)),
                ]
            );
            assert_eq!(grab_mode(&mut app), CursorGrabMode::None);

            click(&mut app, 220, 165);
            assert_eq!(app.should_exit(), Some(AppExit::Success));
        }

        #[test]
        #[expect(
            clippy::too_many_lines,
            reason = "one flow walks the list, the search, the selection and each button"
        )]
        fn select_world_follows_the_list_the_search_and_the_selection() {
            let mut app = title_app();
            let version = mcrs_minecraft_core::VERSION.world_version;
            app.insert_resource(WorldList::Ready(vec![
                world("alpha", "Alpha", false, version),
                world("beta", "Beta", true, version),
                world("gamma", "Gamma", false, version - 1),
            ]));
            click(&mut app, 213, 118);
            assert_eq!(drained::<ReadWorldList>(&mut app), [ReadWorldList]);
            let state = |app: &App| {
                let menu = app.world().resource::<Menu>();
                let Some(WidgetKind::SelectionList(list)) = menu
                    .widgets
                    .get(select_world_screen::LIST)
                    .map(|widget| widget.kind())
                else {
                    panic!("no world list");
                };
                let active = |id| menu.widgets.get(id).unwrap().active;
                (
                    screen(app),
                    list.rows(),
                    list.selected(),
                    active(select_world_screen::PLAY),
                    active(select_world_screen::DELETE),
                )
            };
            let shown =
                |rows, selected, play, delete| (Screen::SelectWorld, rows, selected, play, delete);
            type Step = fn(&mut App);
            let steps: [(Step, _); 7] = [
                (|_| {}, shown(3, None, false, false)),
                (|app| click(app, 213, 60), shown(3, Some(0), true, true)),
                (|app| click(app, 213, 96), shown(3, Some(1), false, true)),
                (|app| click(app, 213, 132), shown(3, Some(2), false, true)),
                (
                    |app| {
                        click(app, 213, 31);
                        type_text(app, "GA");
                    },
                    shown(1, Some(0), false, true),
                ),
                (|app| type_text(app, "x"), shown(0, None, false, false)),
                (
                    |app| {
                        for _ in 0..3 {
                            press(app, KeyCode::Backspace, Key::Backspace, None);
                        }
                    },
                    shown(3, None, false, false),
                ),
            ];
            for (step, expected) in steps {
                step(&mut app);
                assert_eq!(state(&app), expected);
            }

            let alpha = || "alpha".to_owned();
            advance(&mut app, 0);
            let select_alpha = |app: &mut App| {
                advance(app, 1000);
                click(app, 213, 60);
            };
            select_alpha(&mut app);
            click(&mut app, 173, 222);
            assert_eq!(screen(&app), Screen::DeleteWorldConfirmation);
            click(&mut app, 290, 145);
            assert_eq!(state(&app), shown(3, None, false, false));
            assert_eq!(drained::<ReadWorldList>(&mut app), [ReadWorldList]);
            select_alpha(&mut app);
            click(&mut app, 173, 222);
            tap_escape(&mut app);
            assert_eq!(screen(&app), Screen::SelectWorld);
            select_alpha(&mut app);
            click(&mut app, 173, 222);
            click(&mut app, 136, 145);
            assert_eq!(screen(&app), Screen::SelectWorld);
            assert_eq!(
                drained::<DeleteWorld>(&mut app),
                [DeleteWorld { folder_id: alpha() }]
            );

            click(&mut app, 292, 198);
            assert_eq!(
                screen(&app),
                Screen::CreateWorld(CreateWorldParent::SelectWorld)
            );
            tap_escape(&mut app);
            assert_eq!(screen(&app), Screen::SelectWorld);
            assert_eq!(drained::<ReadWorldList>(&mut app), [ReadWorldList]);

            tap_escape(&mut app);
            assert_eq!(screen(&app), Screen::Title);

            click(&mut app, 213, 118);
            select_alpha(&mut app);
            let play = |app: &mut App| {
                press(app, KeyCode::Enter, Key::Enter, None);
                (screen(app), drained::<PlayWorld>(app))
            };
            let play_then_move_down = |app: &mut App| {
                for (key_code, logical_key) in [
                    (KeyCode::Enter, Key::Enter),
                    (KeyCode::ArrowDown, Key::ArrowDown),
                ] {
                    app.world_mut().write_message(KeyboardInput {
                        key_code,
                        logical_key,
                        state: ButtonState::Pressed,
                        text: None,
                        repeat: false,
                        window: Entity::PLACEHOLDER,
                    });
                }
                app.update();
                (screen(app), drained::<PlayWorld>(app))
            };
            let reading = Screen::Message(ScreenMessage::ReadingWorldData);
            assert_eq!(
                [play_then_move_down(&mut app), play(&mut app)],
                [
                    (reading, vec![PlayWorld { folder_id: alpha() }]),
                    (reading, Vec::new())
                ]
            );
        }
    }
}
