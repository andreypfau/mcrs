use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_item_stack::Kind;

use crate::Launch;
use crate::launch::Launching;

pub trait ItemEntityCommandsExt {
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
