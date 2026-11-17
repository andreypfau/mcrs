use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_inventory_client::{InventoryClientSet, MenuAction, MenuInput};
use mcrs_minecraft_network::client::ClientNetworkSystems;

use crate::cursor_grab::grab;

const INVENTORY: KeyCode = KeyCode::KeyE;

pub struct InventoryKeyPlugin;

impl Plugin for InventoryKeyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            toggle_inventory
                .run_if(input_just_pressed(INVENTORY).or_else(input_just_pressed(KeyCode::Escape)))
                .after(ClientNetworkSystems::Receive)
                .before(InventoryClientSet::Predict),
        );
    }
}

fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<Screen>,
    joined: Query<(), With<JoinedGame>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    player: Option<Single<Entity, With<Player>>>,
    mut inputs: MessageWriter<MenuInput>,
) {
    match *screen {
        Screen::None if keys.just_pressed(INVENTORY) && !joined.is_empty() => {
            *screen = Screen::Inventory;
            grab(&mut cursor, false);
        }
        Screen::Inventory if keys.any_just_pressed([INVENTORY, KeyCode::Escape]) => {
            *screen = Screen::None;
            grab(&mut cursor, true);
            if let Some(player) = player {
                inputs.write(MenuInput {
                    viewer: *player,
                    action: MenuAction::Close,
                });
            }
        }
        _ => {}
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
            .add_message::<MenuInput>()
            .add_systems(Update, toggle_inventory);
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
    fn e_opens_the_inventory_and_e_or_escape_closes_it() {
        for close in [KeyCode::KeyE, KeyCode::Escape] {
            let mut app = key_app();
            assert_eq!(
                press(&mut app, KeyCode::KeyE),
                (Screen::Inventory, CursorGrabMode::None)
            );
            assert_eq!(
                press(&mut app, close),
                (Screen::None, CursorGrabMode::Locked),
                "{close:?}"
            );
        }
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
