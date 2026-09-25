// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

mod gbuffer;
mod pass;
mod path;
mod pipelines;

use bevy::core_pipeline::schedule::Core3d;
use bevy::prelude::*;
use bevy::render::{Render, RenderApp, RenderSystems};
use mcrs_minecraft_render::{EffectivePath, RenderPath, WorldPass};

pub struct DeferredPlugin;

impl Plugin for DeferredPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/lighting.wgsl");
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<pipelines::DeferredPipelines>()
            .add_systems(
                Render,
                (
                    pipelines::prepare_deferred_pipelines.in_set(RenderSystems::Prepare),
                    (gbuffer::fit_deferred_frame, path::derive_effective_path)
                        .chain()
                        .in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(
                Core3d,
                (
                    pass::draw_gbuffer
                        .in_set(WorldPass::Opaque)
                        .run_if(resource_equals(EffectivePath(RenderPath::Deferred))),
                    pass::draw_gbuffer_second
                        .in_set(WorldPass::OpaqueSecond)
                        .run_if(resource_equals(EffectivePath(RenderPath::Deferred))),
                    pass::draw_lighting
                        .in_set(WorldPass::Lighting)
                        .run_if(resource_equals(EffectivePath(RenderPath::Deferred))),
                ),
            );
    }
}
