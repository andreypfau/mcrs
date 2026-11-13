use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_item_stack::Kind;

use crate::Launch;
use crate::launch::Launching;

/// The commands that make a stack an item entity.
pub trait ItemEntityCommandsExt {
    /// Puts the stack in the world as an item entity that starts as `launch` says. The stack
    /// leaves its slot. It does nothing to an entity that is not a stack.
    fn launch(&mut self, launch: Launch) -> &mut Self;
}

impl ItemEntityCommandsExt for EntityCommands<'_> {
    fn launch(&mut self, launch: Launch) -> &mut Self {
        self.drop_at(launch.form.position())
            .queue_silenced(move |mut entity: EntityWorldMut| {
                if entity.contains::<Kind>() {
                    entity.insert((
                        launch.values,
                        Launching {
                            source: launch.source,
                            form: launch.form,
                        },
                    ));
                }
            })
    }
}
