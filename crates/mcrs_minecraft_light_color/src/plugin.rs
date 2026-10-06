use bevy_app::{App, Plugin, Startup};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, Res};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_registry::shared::Resolved;
use mcrs_minecraft_world::resolvers::AddRegistryResolver;

use crate::colors::{LightColors, LightType};
use crate::item::{ItemLights, LightColorIds};

pub struct LightColorPlugin;

impl Plugin for LightColorPlugin {
    fn build(&self, app: &mut App) {
        app.add_registry_resolver(LightColorIds::resolve);
        app.add_systems(Startup, (insert_light_colors, insert_item_lights));
    }
}

fn insert_light_colors(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    blocks: Res<Blocks>,
    registries: Res<RegistrySet>,
) {
    let tags = registries
        .tags::<Block>()
        .expect("the loaded registries hold the block tags");
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
    registries: Res<RegistrySet>,
    ids: Res<Resolved<LightColorIds>>,
) {
    let lights = ItemLights::load(&asset_server, &blocks, &items, &registries, &ids)
        .unwrap_or_else(|e| panic!("{e}"));
    tracing::info!(items = lights.mapped_count(), "loaded item lights");
    commands.insert_resource(lights);
}
