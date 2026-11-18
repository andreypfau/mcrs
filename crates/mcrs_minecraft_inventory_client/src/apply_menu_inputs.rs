use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Commands;
use bevy_ecs::world::World;

use crate::MenuAction;
use crate::MenuInput;
use crate::close_menu::close_menu;
use crate::predict_click::predict_click;

pub(crate) fn apply_menu_inputs(mut inputs: MessageReader<MenuInput>, mut commands: Commands) {
    for input in inputs.read() {
        let viewer = input.viewer;
        match input.action {
            MenuAction::Click(click) => {
                commands.queue(move |world: &mut World| predict_click(world, viewer, click));
            }
            MenuAction::Close => {
                commands.queue(move |world: &mut World| {
                    world.run_system_cached_with(close_menu, viewer).ok();
                });
            }
        }
    }
}
