use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::settings::WgpuLimits;
use mcrs_minecraft_client_world::registries::SessionSet;
use mcrs_minecraft_client_world::session::session_boundary;
use mcrs_minecraft_entity_render::{EntityAssets, EntityRenderPlugin};
use mcrs_minecraft_registries::CurrentRegistries;

use crate::stream::BlockCatalog;

pub struct ClientEntityRenderPlugin;

impl Plugin for ClientEntityRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EntityRenderPlugin, crate::player_skins::PlayerSkinsPlugin))
            .add_systems(
                Update,
                (
                    clear_entity_assets
                        .in_set(SessionSet::Clear)
                        .run_if(session_boundary),
                    build_entity_assets
                        .after(SessionSet::Clear)
                        .run_if(resource_exists::<CurrentRegistries>)
                        .run_if(mcrs_minecraft_entity_render::entity_passes),
                ),
            );
    }
}

fn clear_entity_assets(mut commands: Commands) {
    commands.remove_resource::<EntityAssets>();
}

#[derive(Default)]
struct Built {
    registries: Option<bevy::ecs::change_detection::Tick>,
    generation: u32,
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::per_frame_insert_resource,
        reason = "the early return skips the insert unless the registries changed since the last build"
    )
)]
fn build_entity_assets(
    mut commands: Commands,
    catalog: Option<Res<BlockCatalog>>,
    registries: Res<CurrentRegistries>,
    device: Option<Res<RenderDevice>>,
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
    built.generation = built.generation.wrapping_add(1);
    entity_assets.generation = built.generation;
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
