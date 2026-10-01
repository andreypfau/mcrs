// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

mod counts;
mod heat;
pub mod probe;
mod readback;

use bevy::core_pipeline::schedule::Core3d;
use bevy::prelude::*;
use bevy::render::view::window::prepare_windows;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use mcrs_minecraft_render::WorldPass;

use crate::counts::ArgsReadback;
use crate::heat::{Heat, HeatWorkgroups};
use crate::probe::{CpuTimings, GpuTimings};

pub use counts::{DrawnTriangles, ReportedCounts};

pub struct ProbePlugin {
    /// Workgroups of arithmetic burnt each frame to keep the GPU clocked up while passes are timed.
    pub heat: Option<u32>,
    pub timestamps: bool,
}

impl Plugin for ProbePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/heat.wgsl");

        let triangles = DrawnTriangles::default();
        let timings = GpuTimings::default();
        let cpu = CpuTimings::default();
        let reported = ReportedCounts::default();
        app.insert_resource(triangles.clone())
            .insert_resource(timings.clone())
            .insert_resource(cpu.clone())
            .insert_resource(reported.clone())
            .add_systems(First, probe::frame_started.after(bevy::time::TimeSystems))
            .add_systems(Last, probe::main_ended)
            .add_systems(PostStartup, probe::log_system_counts);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(triangles)
            .insert_resource(timings)
            .insert_resource(cpu)
            .insert_resource(reported)
            .insert_resource(probe::TimePasses(self.timestamps))
            .add_systems(RenderStartup, (probe::init, probe::log_system_counts))
            .add_systems(
                Render,
                (
                    probe::extracted.before(RenderSystems::ExtractCommands),
                    probe::acquiring.before(prepare_windows),
                    probe::acquired.after(prepare_windows),
                    probe::prepared
                        .after(RenderSystems::Prepare)
                        .before(RenderSystems::Render),
                    probe::rendered
                        .after(RenderSystems::Render)
                        .before(RenderSystems::Cleanup),
                    probe::cleaned.in_set(RenderSystems::PostCleanup),
                    counts::init_args_readback
                        .in_set(RenderSystems::Prepare)
                        .run_if(not(resource_exists::<ArgsReadback>)),
                    counts::read_draw_args.in_set(RenderSystems::Cleanup),
                    counts::report_counts.in_set(RenderSystems::Cleanup),
                    probe::read.in_set(RenderSystems::Cleanup),
                ),
            )
            .add_systems(Core3d, probe::begin_frame.in_set(WorldPass::Upload));
        if let Some(workgroups) = self.heat {
            render_app
                .insert_resource(HeatWorkgroups(workgroups))
                .add_systems(
                    Render,
                    heat::init_heat
                        .in_set(RenderSystems::Prepare)
                        .run_if(not(resource_exists::<Heat>)),
                );
        }
    }
}
