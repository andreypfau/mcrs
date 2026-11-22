use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_widget::WidgetEvent;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::player_killed::PlayerKilled;
use mcrs_minecraft_client_world::request_respawn::RequestRespawn;
use mcrs_minecraft_client_world::respawn_screen::RespawnScreen;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_network::client::LeaveRequest;

use crate::cursor_grab::grab;
use crate::death_screen::{self, DeathScreen};
use crate::menu::Menu;
use crate::menu_event::MenuEvent;
use crate::menu_input_systems::MenuInputSystems;

pub struct DeathScreenPlugin;

impl Plugin for DeathScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerKilled>()
            .add_message::<RequestRespawn>()
            .add_message::<LeaveRequest>()
            .add_systems(
                PreUpdate,
                follow_death_screen_buttons
                    .after(MenuInputSystems)
                    .run_if(on_message::<MenuEvent>),
            )
            .add_systems(
                Update,
                (
                    open_death_screen.run_if(on_message::<PlayerKilled>),
                    close_death_screen.run_if(
                        any_match_filter::<(With<Player>, Changed<Vitals>)>
                            .and_then(resource_equals(Screen::Death)),
                    ),
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                count_death_screen_ticks.run_if(
                    resource_exists::<DeathScreen>.and_then(resource_equals(Screen::Death)),
                ),
            );
    }
}

fn open_death_screen(
    mut kills: MessageReader<PlayerKilled>,
    mut screen: ResMut<Screen>,
    cursor: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
    mut commands: Commands,
) {
    let Some(killed) = kills.read().last() else {
        return;
    };
    if killed.respawn_screen != RespawnScreen::Shown {
        return;
    }
    commands.insert_resource(DeathScreen::new(killed.message.clone()));
    *screen = Screen::Death;
    if let Some(mut cursor) = cursor {
        grab(&mut cursor, false);
    }
}

fn close_death_screen(
    player: Query<&Vitals, With<Player>>,
    mut screen: ResMut<Screen>,
    cursor: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
) {
    if !player.single().is_ok_and(Vitals::is_alive) {
        return;
    }
    *screen = Screen::None;
    if let Some(mut cursor) = cursor {
        grab(&mut cursor, true);
    }
}

fn count_death_screen_ticks(mut death: ResMut<DeathScreen>, menu: Option<ResMut<Menu>>) {
    let was_active = death.buttons_active();
    death.tick();
    if was_active || !death.buttons_active() {
        return;
    }
    let Some(mut menu) = menu else {
        return;
    };
    for id in [death_screen::RESPAWN, death_screen::TITLE_SCREEN] {
        if let Some(button) = menu.widgets.get_mut(id) {
            button.active = true;
        }
    }
}

fn follow_death_screen_buttons(
    mut events: MessageReader<MenuEvent>,
    screen: Res<Screen>,
    mut respawn: MessageWriter<RequestRespawn>,
    mut leave: MessageWriter<LeaveRequest>,
    mut menu: Option<ResMut<Menu>>,
) {
    for event in events.read() {
        if *screen != Screen::Death {
            continue;
        }
        let MenuEvent::Widget(WidgetEvent::Activated { widget, .. }) = *event else {
            continue;
        };
        match widget {
            death_screen::RESPAWN => {
                respawn.write(RequestRespawn);
            }
            death_screen::TITLE_SCREEN => {
                if let Some(button) = menu.as_mut().and_then(|menu| menu.widgets.get_mut(widget)) {
                    button.active = false;
                }
                leave.write(LeaveRequest);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_protocol_game::text::Text;

    use super::*;
    use crate::menu_test_app::{caption, click, drained, game_app, screen, tap_escape};

    const RESPAWN_BUTTON: (i32, i32) = (200, 140);

    #[test]
    fn the_respawn_button_waits_twenty_ticks_and_the_screen_closes_when_the_player_lives() {
        let mut app = game_app();
        app.add_plugins(DeathScreenPlugin);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<Player>>()
            .single(app.world())
            .unwrap();
        app.world_mut().entity_mut(player).insert(Vitals {
            health: 0.0,
            death_time: Some(0),
            ..Vitals::default()
        });
        app.world_mut().write_message(PlayerKilled {
            message: Text::text("Steve fell from a high place"),
            respawn_screen: RespawnScreen::Shown,
        });
        app.update();
        app.update();
        assert_eq!(screen(&app), Screen::Death);
        assert_eq!(
            caption(&app, death_screen::MESSAGE),
            "Steve fell from a high place"
        );

        click(&mut app, RESPAWN_BUTTON.0, RESPAWN_BUTTON.1);
        let early = drained::<RequestRespawn>(&mut app);
        for _ in 0..20 {
            app.world_mut().run_schedule(FixedUpdate);
        }
        click(&mut app, RESPAWN_BUTTON.0, RESPAWN_BUTTON.1);
        assert_eq!(
            (early, drained::<RequestRespawn>(&mut app)),
            (Vec::new(), vec![RequestRespawn])
        );

        tap_escape(&mut app);
        assert_eq!(screen(&app), Screen::Death);
        app.world_mut().entity_mut(player).insert(Vitals {
            health: 20.0,
            ..Vitals::default()
        });
        app.update();
        assert_eq!(screen(&app), Screen::None);
    }

    #[test]
    fn a_kill_and_a_living_player_in_one_frame_leave_no_death_screen() {
        let mut app = game_app();
        app.add_plugins(DeathScreenPlugin);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<Player>>()
            .single(app.world())
            .unwrap();
        app.world_mut().write_message(PlayerKilled {
            message: Text::text("Steve fell from a high place"),
            respawn_screen: RespawnScreen::Shown,
        });
        app.world_mut().entity_mut(player).insert(Vitals::new(20.0));
        app.update();
        app.update();
        assert_eq!(screen(&app), Screen::None);
    }
}
