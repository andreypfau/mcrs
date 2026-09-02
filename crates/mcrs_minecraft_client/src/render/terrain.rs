use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::arenas::Arenas;
use super::binds::Bindings;
use super::draws::DrawList;
use super::frame::Frame;
use super::pipeline::Pipelines;
use super::shaders::Shaders;
use super::sprites::Sprites;
use super::{Budget, TerrainBudget};

#[derive(Resource)]
pub(super) struct Terrain {
    pub budget: Arc<Budget>,
    pub arenas: Arenas,
    pub frame: Frame,
    pub sprites: Sprites,
    pub binds: Bindings,
    pub pipelines: Pipelines,
    pub list: DrawList,
    pub cull_grid: u32,
}

impl Terrain {
    /// Sizes the visible list to the draws as they stand and restates the
    /// per-draw parameters, so a world that has grown is drawn whole rather
    /// than up to a share of a fixed list.
    pub fn rebuild_params(&mut self, device: &RenderDevice, pipeline_cache: &PipelineCache) {
        self.list.rebuild(self.sprites.animated_from);
        if self.arenas.grow_visible(self.list.visible_entries, device) {
            self.binds
                .rebuild_cull(&self.arenas, &self.frame, device, pipeline_cache);
            self.binds
                .rebuild_draw(&self.arenas, &self.sprites, device, pipeline_cache);
        }
    }
}

pub(super) fn init_terrain(
    mut commands: Commands,
    budget: Res<TerrainBudget>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let budget = budget.0.clone();
    let arenas = Arenas::new(&budget, &device);
    let frame = Frame::new(&budget, &device);
    let sprites = Sprites::new(&budget, &device, &queue);
    let binds = Bindings::new(&arenas, &frame, &sprites, &device, &pipeline_cache);
    let pipelines = Pipelines::new(Shaders::load(&asset_server), &binds, &pipeline_cache);

    commands.insert_resource(Terrain {
        cull_grid: super::pass::cull_grid(&device.limits()),
        list: DrawList::new(),
        budget,
        arenas,
        frame,
        sprites,
        binds,
        pipelines,
    });
}

pub(super) fn write_sky(
    terrain: Option<Res<Terrain>>,
    sky: Option<Res<crate::sky_render::ExtractedSky>>,
    queue: Res<RenderQueue>,
) {
    let (Some(terrain), Some(sky)) = (terrain, sky) else {
        return;
    };
    queue.write_buffer(&terrain.frame.sky, 0, bytemuck::bytes_of(&sky.uniform));
}
