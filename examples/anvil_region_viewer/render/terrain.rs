use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::arenas::Arenas;
use super::binds::{Bindings, SkyTextures};
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
    let sky_textures = SkyTextures::new(&layout.celestials, &layout.clouds, &device, &queue);
    let binds = Bindings::new(
        &arenas,
        &frame,
        &sprites,
        &sky_textures,
        &device,
        &pipeline_cache,
    );
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
