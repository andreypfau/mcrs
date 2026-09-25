use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::render_resource::{BindGroup, BindGroupLayoutDescriptor, PipelineCache};
use bevy::render::renderer::{RenderDevice, RenderQueue};

use crate::sky::ExtractedSky;

use super::Brightness;
use super::arenas::Arenas;
use super::binds::Bindings;
use super::draws::DrawList;
use super::frame::Frame;
use super::hiz::Hiz;
use super::pipeline::Pipelines;
use super::shaders::Shaders;
use super::sprites::Sprites;
use super::texture;
use super::{Budget, TerrainBudget};

#[derive(Resource)]
pub struct Terrain {
    pub(super) budget: Arc<Budget>,
    pub(super) arenas: Arenas,
    pub(super) frame: Frame,
    pub sprites: Sprites,
    pub(super) binds: Bindings,
    pub(super) pipelines: Pipelines,
    pub(super) list: DrawList,
    pub(super) cull_grid: u32,
    pub(super) hiz: Hiz,
}

impl Terrain {
    pub fn view_layout(&self) -> &BindGroupLayoutDescriptor {
        &self.binds.view_layout
    }

    pub fn draw_layout(&self) -> &BindGroupLayoutDescriptor {
        &self.binds.draw_layout
    }

    pub fn view_bind_group(&self) -> &BindGroup {
        &self.binds.view
    }

    pub fn draw_bind_group(&self) -> &BindGroup {
        &self.binds.draw
    }

    pub fn write_cave_visibility(&self, queue: &RenderQueue, bits: &[u32]) {
        queue.write_buffer(&self.frame.cave, 0, bytemuck::cast_slice(bits));
    }

    /// Sizes the visible list to the draws as they stand and restates the
    /// per-draw parameters, so a world that has grown is drawn whole rather
    /// than up to a share of a fixed list.
    pub(crate) fn rebuild_params(&mut self, device: &RenderDevice, pipeline_cache: &PipelineCache) {
        self.list.rebuild();
        if self.arenas.grow_visible(self.list.visible_entries, device) {
            self.binds.rebuild_cull(
                &self.arenas,
                &self.frame,
                &self.hiz.view,
                device,
                pipeline_cache,
            );
            self.binds.rebuild_draw(
                &self.arenas,
                &self.frame,
                &self.sprites,
                device,
                pipeline_cache,
            );
        }
    }
}

pub(super) fn init_terrain(
    mut commands: Commands,
    budget: Res<TerrainBudget>,
    device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let budget = budget.0.clone();
    let arenas = Arenas::new(&budget, &device);
    let frame = Frame::new(&budget, &device);
    let sprites = Sprites::new(&budget, &device);
    let hiz = Hiz::new(&device, &asset_server, &pipeline_cache);
    let binds = Bindings::new(
        &arenas,
        &frame,
        &sprites,
        &hiz.view,
        &device,
        &pipeline_cache,
    );
    let pipelines = Pipelines::new(Shaders::load(&asset_server), &binds, &pipeline_cache);

    commands.insert_resource(super::upload::Staging::new(&device, budget.upload));
    commands.insert_resource(Terrain {
        cull_grid: super::pass::cull_grid(&device.limits()),
        list: DrawList::new(),
        budget,
        arenas,
        frame,
        sprites,
        binds,
        pipelines,
        hiz,
    });
}

/// The lightmap follows the three light colours and the brightness alone, which move at tick rate
/// and often not at all, so a frame whose inputs match the last one uploads nothing.
pub(super) fn write_lightmap(
    terrain: Option<Res<Terrain>>,
    sky: Option<Res<ExtractedSky>>,
    brightness: Res<Brightness>,
    queue: Res<RenderQueue>,
    mut last: Local<Option<([[f32; 4]; 3], f32)>>,
) {
    let (Some(terrain), Some(sky)) = (terrain, sky) else {
        return;
    };
    let lights = [
        sky.uniform.ambient,
        sky.uniform.sky_light,
        sky.uniform.block_light,
    ];
    if *last == Some((lights, brightness.0)) {
        return;
    }
    *last = Some((lights, brightness.0));
    texture::write_lightmap(
        &terrain.sprites.lightmap,
        &queue,
        &sky.uniform,
        brightness.0,
    );
}
