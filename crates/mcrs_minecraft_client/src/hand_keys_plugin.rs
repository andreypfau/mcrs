use bevy::prelude::*;
use mcrs_minecraft_client_gui::item_key_codes;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_inventory_client::HandInput;
use mcrs_minecraft_player_holders::HandAction;

use crate::input;

pub struct HandKeysPlugin;

impl Plugin for HandKeysPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            RunFixedMainLoop,
            read_hand_keys
                .run_if(input::cursor_grabbed_without_screen)
                .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
        );
    }
}

fn read_hand_keys(
    keys: Res<ButtonInput<KeyCode>>,
    player: Single<Entity, With<Player>>,
    mut inputs: MessageWriter<HandInput>,
) {
    let drop = if keys.any_pressed(item_key_codes::CONTROL) {
        HandAction::DropStack
    } else {
        HandAction::DropOne
    };
    for (key, action) in [
        (item_key_codes::DROP, drop),
        (item_key_codes::SWAP_OFFHAND, HandAction::SwapHands),
    ] {
        if keys.just_pressed(key) {
            inputs.write(HandInput {
                viewer: *player,
                action,
            });
        }
    }
}
