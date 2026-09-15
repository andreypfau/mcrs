use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::tag::TagPhase;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::light::{BlockLightRegistry, block_light_registry};
use mcrs_minecraft_world::transition_to_playing;

pub struct BlockLightTablePlugin;

impl Plugin for BlockLightTablePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::Lighting>();
        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            insert_block_light_registry
                .after(TagPhase::Freeze)
                .before(transition_to_playing)
                .run_if(bevy_ecs::schedule::common_conditions::resource_equals(
                    crate::Lighting::Propagated,
                )),
        );
    }
}

fn insert_block_light_registry(mut commands: Commands, blocks: Res<Blocks>) {
    commands.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}
