// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

mod gbuffer;
mod pass;
mod path;
mod pipelines;
mod reconstruct;
mod views;

use bevy::core_pipeline::schedule::Core3d;
use bevy::prelude::*;
use bevy::render::{Render, RenderApp, RenderSystems};
use mcrs_minecraft_render::{DebugViews, EffectivePath, Occlusion, RenderPath, WorldPass};

pub struct DeferredPlugin {
    pub parity_mask: bool,
}

#[derive(Resource, Clone, Copy)]
pub(crate) struct ParityMask(pub bool);

impl Plugin for DeferredPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/lighting.wgsl");
        bevy::asset::embedded_asset!(app, "shaders/show.wgsl");
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(ParityMask(self.parity_mask))
            .init_resource::<pipelines::DeferredPipelines>()
            .add_systems(
                Render,
                (
                    pipelines::prepare_deferred_pipelines.in_set(RenderSystems::Prepare),
                    (
                        gbuffer::fit_deferred_frame,
                        reconstruct::write_lighting_uniform,
                        path::derive_effective_path,
                    )
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
                    pass::draw_forward_deferred
                        .in_set(WorldPass::Forward)
                        .run_if(resource_equals(EffectivePath(RenderPath::Deferred)))
                        .run_if(pass::draws_forward),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let occlusion = app.world().resource::<Occlusion>().0;
        let mut views = app.world_mut().resource_mut::<DebugViews>();
        let deferred = views::DeferredViews::register(&mut views, occlusion);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(deferred);
        }
    }
}
