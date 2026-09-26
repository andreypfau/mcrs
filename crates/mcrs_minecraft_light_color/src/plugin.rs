use bevy_app::{App, Plugin};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::tag::{DynTagRegistry, TagPhase};
use mcrs_minecraft_block::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::VoxelId;

use crate::colors::{LightColors, LightType};

pub struct LightColorPlugin;

impl Plugin for LightColorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            insert_light_colors.after(TagPhase::Freeze),
        );
    }
}

fn insert_light_colors(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    blocks: Res<Blocks>,
    tags: Res<DynTagRegistry<Block>>,
) {
    let colors = LightColors::load(&asset_server, &blocks, &tags).unwrap_or_else(|e| panic!("{e}"));
    let coloured_states = (0..blocks.state_count())
        .filter(|&id| colors.light_type(VoxelId(id as u16)) != LightType::DEFAULT)
        .count();
    tracing::info!(
        colours = colors.type_count() - 1,
        coloured_states,
        "loaded light colours"
    );
    commands.insert_resource(colors);
}
