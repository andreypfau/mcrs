// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

mod colour;
mod volume;

use bevy::core_pipeline::schedule::Core3d;
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResourcePlugin;
use bevy::render::render_resource::BindGroup;
use bevy::render::{Render, RenderApp, RenderSystems};
use mcrs_minecraft_render::{LightTint, WorldPass};

pub use volume::{VolumeCommand, VolumeQueue, VolumeSettings};

pub struct DeferredPlugin;

impl Plugin for DeferredPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/colour.wgsl");
        let queue = VolumeQueue::default();
        app.init_resource::<VolumeSettings>()
            .insert_resource(queue.clone())
            .add_plugins(ExtractResourcePlugin::<VolumeSettings>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(queue)
            .insert_resource(LightTint {
                layout: volume::volume_layout(),
                bind_group: None,
            })
            .init_resource::<colour::ColourPipelines>()
            .add_systems(
                Render,
                (
                    colour::prepare_colour_pipelines.in_set(RenderSystems::Prepare),
                    (volume::apply_volume_commands, volume::write_volume_terms)
                        .chain()
                        .in_set(RenderSystems::Prepare),
                    (volume::fit_volume, publish_tint)
                        .chain()
                        .in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(Core3d, colour::propagate_colour.in_set(WorldPass::Upload));
    }
}

fn publish_tint(volume: Option<Res<volume::Volume>>, mut tint: ResMut<LightTint>) {
    let bind_group = volume.as_deref().map(|volume| &volume.bind_group);
    if bind_group.map(BindGroup::id) != tint.bind_group.as_ref().map(BindGroup::id) {
        tint.bind_group = bind_group.cloned();
    }
}
