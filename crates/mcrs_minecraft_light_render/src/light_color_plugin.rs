use bevy::asset::io::AssetSourceId;
use bevy::prelude::*;
use mcrs_minecraft_assets::asset::read_json_corpus;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_light_color::colors::{CORPUS_DIRECTORY, LightColors, LightType};
use mcrs_minecraft_light_color::item::{self, ItemLights};
use mcrs_minecraft_registries::CurrentRegistries;

/// The light a held item emits, rebuilt with the registries.
#[derive(Resource, Clone, Debug)]
pub struct HeldItemLights(pub ItemLights);

/// Loads the light colour table and the held item lights each time the
/// registries change. Needs [`Blocks`], [`Items`] and an [`AssetServer`].
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

fn read_corpus(asset_server: &AssetServer, directory: &str) -> Option<Vec<(String, Vec<u8>)>> {
    let Ok(source) = asset_server.get_source(AssetSourceId::Default) else {
        error!(directory, "the default asset source is missing");
        return None;
    };
    match read_json_corpus(source.reader(), directory) {
        Ok(files) => Some(files),
        Err(error) => {
            error!(%error, directory, "the corpus does not read");
            None
        }
    }
}

fn rebuild_light_colors(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
) {
    let Some(tags) = registries.tags::<Block>() else {
        error!(
            "the registries declare no block registry with its tags; light colours stay as they were"
        );
        return;
    };
    let Some(files) = read_corpus(&asset_server, CORPUS_DIRECTORY) else {
        return;
    };
    let colors = match LightColors::from_files(files, &blocks, &tags) {
        Ok(colors) => colors,
        Err(error) => {
            error!(%error, "the light colours do not load; they stay as they were");
            return;
        }
    };
    let coloured_states = (0..blocks.state_count())
        .filter(|&id| colors.light_type(VoxelId(id as u16)) != LightType::DEFAULT)
        .count();
    info!(
        colours = colors.type_count() - 1,
        coloured_states, "loaded light colours"
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
    let Some(files) = read_corpus(&asset_server, item::CORPUS_DIRECTORY) else {
        return;
    };
    let lights = match ItemLights::from_files(files, &blocks, &items, &registries) {
        Ok(lights) => lights,
        Err(error) => {
            error!(%error, "the item lights do not load; they stay as they were");
            return;
        }
    };
    info!(items = lights.mapped_count(), "loaded item lights");
    commands.insert_resource(HeldItemLights(lights));
}
