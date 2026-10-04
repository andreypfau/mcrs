use bevy_app::{App, Plugin};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res};
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::tag::{DynTagRegistry, TagPhase};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_item::{Item, Items};
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_registry::key::Fluid;

use crate::colors::{LightColors, LightType};
use crate::item::ItemLights;

pub struct LightColorPlugin;

impl Plugin for LightColorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            (insert_light_colors, insert_item_lights).after(TagPhase::Freeze),
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

fn insert_item_lights(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    blocks: Res<Blocks>,
    items: Res<Items>,
    item_tags: Res<DynTagRegistry<Item>>,
    fluid_tags: Res<DynTagRegistry<Fluid>>,
) {
    let lights = ItemLights::load(&asset_server, &blocks, &items, &item_tags, &fluid_tags)
        .unwrap_or_else(|e| panic!("{e}"));
    tracing::info!(items = lights.mapped_count(), "loaded item lights");
    commands.insert_resource(lights);
}
