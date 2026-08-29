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
use super::{Layout, WorldLayout};

#[derive(Resource)]
pub(super) struct Terrain {
    pub layout: Arc<Layout>,
    pub arenas: Arenas,
    pub frame: Frame,
    pub sprites: Sprites,
    pub binds: Bindings,
    pub pipelines: Pipelines,
    pub list: DrawList,
}

impl Terrain {
    pub fn rebuild_params(&mut self) {
        self.list.rebuild(&self.layout, self.sprites.animated_from);
    }
}

pub(super) fn init_terrain(
    mut commands: Commands,
    layout: Res<WorldLayout>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = layout.0.clone();
    let arenas = Arenas::new(&layout, &device);
    let frame = Frame::new(&layout, &device);
    let sprites = Sprites::new(&layout, &device, &queue);
    let binds = Bindings::new(&arenas, &frame, &sprites, &device, &pipeline_cache);
    let pipelines = Pipelines::new(Shaders::load(&asset_server), &binds, &pipeline_cache);

    commands.insert_resource(Terrain {
        list: DrawList::new(layout.max_draws().max(1)),
        layout,
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
