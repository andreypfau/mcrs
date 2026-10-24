use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Commands, Res};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_light::block_light::{BlockLightRegistry, block_light_registry};

/// Light propagation in every dimension. Without it the block light table is never built, so
/// no dimension registers a lighting engine and every column goes to the client at full sky
/// light: sunlight, torches and shadows stop existing, and the world loads without the
/// propagation cost.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        crate::require::<Self, crate::ServerCorePlugin>(app);
        app.add_systems(Startup, insert_block_light_registry);
    }
}

fn insert_block_light_registry(mut commands: Commands, blocks: Res<Blocks>) {
    commands.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}
