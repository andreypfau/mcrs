use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::light::{BlockLightRegistry, block_light_registry};

pub struct BlockLightTablePlugin;

impl Plugin for BlockLightTablePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::Lighting>();
        app.add_systems(
            Startup,
            insert_block_light_registry.run_if(
                bevy_ecs::schedule::common_conditions::resource_equals(crate::Lighting::Propagated),
            ),
        );
    }
}

fn insert_block_light_registry(mut commands: Commands, blocks: Res<Blocks>) {
    commands.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}
