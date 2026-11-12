//! The glue between the entity passes and the client: it builds the entity assets once the
//! resource pack and the synced registries are in.

use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::settings::WgpuLimits;
use mcrs_minecraft_entity_render::{EntityAssets, EntityRenderPlugin};
use mcrs_minecraft_registries::CurrentRegistries;

use crate::stream::BlockCatalog;

/// Draws the client entities with the entity passes, once the resource pack and the synced
/// registries are in.
pub struct ClientEntityRenderPlugin;

impl Plugin for ClientEntityRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EntityRenderPlugin).add_systems(
            Update,
            build_entity_assets
                .run_if(resource_exists::<CurrentRegistries>)
                .run_if(mcrs_minecraft_entity_render::entity_passes),
        );
    }
}

/// The registries and the pack that the assets were built from.
#[derive(Default)]
struct Built {
    registries: Option<bevy::ecs::change_detection::Tick>,
}

fn build_entity_assets(
    mut commands: Commands,
    catalog: Option<Res<BlockCatalog>>,
    registries: Res<CurrentRegistries>,
    device: Option<Res<RenderDevice>>,
    assets: Option<Res<EntityAssets>>,
    mut built: Local<Built>,
) {
    let Some(resources) = catalog.as_deref().and_then(BlockCatalog::pack) else {
        return;
    };
    let changed = registries.last_changed();
    if built.registries == Some(changed) {
        return;
    }
    built.registries = Some(changed);
    let array_layers = device.map_or(WgpuLimits::default().max_texture_array_layers, |device| {
        device.limits().max_texture_array_layers
    });
    let mut entity_assets = EntityAssets::build(resources, &registries, array_layers);
    entity_assets.generation = assets.map_or(0, |assets| assets.generation.wrapping_add(1));
    info!(
        definitions = entity_assets.library.definitions.len(),
        textures = entity_assets.library.textures.len(),
        diagnostics = entity_assets.pack.diagnostics.len(),
        "entity assets built"
    );
    for diagnostic in &entity_assets.pack.diagnostics {
        warn!("{diagnostic}");
    }
    commands.insert_resource(entity_assets);
}
