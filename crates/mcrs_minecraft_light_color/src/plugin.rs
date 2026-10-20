use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, Res, resource_exists_and_changed};
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::colors::{LightColors, LightType};
use crate::item::ItemLights;

pub struct LightColorPlugin;

impl Plugin for LightColorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (rebuild_light_colors, rebuild_item_lights)
                .run_if(resource_exists_and_changed::<CurrentRegistries>),
        );
    }
}

fn rebuild_light_colors(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
) {
    let Some(tags) = registries.tags::<Block>() else {
        tracing::error!(
            "the registries declare no block registry with its tags; light colours stay as they were"
        );
        return;
    };
    let colors = match LightColors::load(&asset_server, &blocks, &tags) {
        Ok(colors) => colors,
        Err(error) => {
            tracing::error!(%error, "the light colours do not load; they stay as they were");
            return;
        }
    };
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

fn rebuild_item_lights(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    blocks: Res<Blocks>,
    items: Res<Items>,
    registries: Res<CurrentRegistries>,
) {
    let lights = match ItemLights::load(&asset_server, &blocks, &items, &registries) {
        Ok(lights) => lights,
        Err(error) => {
            tracing::error!(%error, "the item lights do not load; they stay as they were");
            return;
        }
    };
    tracing::info!(items = lights.mapped_count(), "loaded item lights");
    commands.insert_resource(lights);
}
