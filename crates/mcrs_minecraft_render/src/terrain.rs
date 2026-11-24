use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroup, BindGroupLayoutDescriptor, Buffer, PipelineCache, TextureView,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};

use crate::sky::ExtractedSky;

use super::arenas::Arenas;
use super::binds::Bindings;
use super::cull::Cull;
use super::draws::DrawList;
use super::frame::Frame;
use super::render_path::RenderPath;
use super::shaders::Shaders;
use super::sprites::Sprites;
use super::texture;
use super::{Brightness, Budget, Occlusion, TerrainBudget};

#[derive(Resource)]
pub struct Terrain {
    pub(super) path: RenderPath,
    pub(super) budget: Arc<Budget>,
    pub(super) arenas: Arenas,
    pub(super) frame: Frame,
    pub sprites: Sprites,
    pub(super) binds: Bindings,
    pub(super) shaders: Shaders,
    pub(super) list: DrawList,
    pub(super) cull: Option<Cull>,
}

impl Terrain {
    pub fn path(&self) -> RenderPath {
        self.path
    }

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

    pub fn depth_pyramid(&self) -> Option<&TextureView> {
        self.cull.as_ref().map(|cull| &cull.hiz.view)
    }

    pub fn depth_pyramid_levels(
        &self,
        occlusion: &Occlusion,
        pipeline_cache: &PipelineCache,
    ) -> u32 {
        match &self.cull {
            Some(cull) if self.second_pass(occlusion, pipeline_cache) => cull.hiz.levels(),
            _ => 0,
        }
    }

    pub(super) fn hiz_levels(&self, occlusion: &Occlusion) -> u32 {
        match &self.cull {
            Some(cull) if occlusion.0 => cull.hiz.levels(),
            _ => 0,
        }
    }

    pub fn draw_args(&self) -> Option<&Buffer> {
        self.cull.as_ref().map(Cull::args)
    }

    pub fn second_pass(&self, occlusion: &Occlusion, pipeline_cache: &PipelineCache) -> bool {
        occlusion.0
            && self
                .cull
                .as_ref()
                .is_some_and(|cull| cull.second_pass(&self.list, pipeline_cache))
    }

    pub fn write_cave_visibility(&self, queue: &RenderQueue, bits: &[u32]) {
        if let Some(cull) = &self.cull {
            queue.write_buffer(cull.cave(), 0, bytemuck::cast_slice(bits));
        }
    }

    pub(super) fn rebuild_draw(&mut self, device: &RenderDevice, pipeline_cache: &PipelineCache) {
        self.binds.rebuild_draw(
            &self.arenas,
            self.cull.as_ref(),
            &self.sprites,
            device,
            pipeline_cache,
        );
    }

    pub(crate) fn rebuild_params(&mut self, device: &RenderDevice, pipeline_cache: &PipelineCache) {
        if let Err(kind) = self.list.rebuild() {
            error!(
                ?kind,
                "cannot rebuild draw params with duplicate DrawKind; each kind owns one args row"
            );
            return;
        }
        let entries = self.list.visible_entries;
        let Some(cull) = self
            .cull
            .as_mut()
            .filter(|cull| !cull.holds_visible(entries))
        else {
            return;
        };
        cull.grow_visible(entries, device);
        cull.rebind(&self.arenas, device, pipeline_cache);
        self.rebuild_draw(device, pipeline_cache);
    }
}

pub(super) fn init_terrain(
    mut commands: Commands,
    budget: Res<TerrainBudget>,
    path: Res<RenderPath>,
    device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let (path, budget) = (*path, budget.0.clone());
    info!(?path, "terrain is drawn on this path");
    let arenas = Arenas::new(path, &budget, &device);
    let frame = Frame::new(&device);
    let sprites = Sprites::new(path, &budget, &device);
    let cull = path.is_full().then(|| {
        Cull::new(
            &budget,
            &arenas,
            &super::shared_bindings::terrain_view::layout(),
            &device,
            &asset_server,
            &pipeline_cache,
        )
    });
    let binds = Bindings::new(
        path,
        &frame,
        &arenas,
        cull.as_ref(),
        &sprites,
        &device,
        &pipeline_cache,
    );

    commands.insert_resource(super::upload::Staging::new(&device, budget.upload));
    commands.insert_resource(Terrain {
        path,
        list: DrawList::new(),
        shaders: Shaders::load(&asset_server),
        budget,
        arenas,
        frame,
        sprites,
        binds,
        cull,
    });
}

pub(super) fn write_lightmap(
    If(terrain): If<Res<Terrain>>,
    If(sky): If<Res<ExtractedSky>>,
    brightness: Res<Brightness>,
    queue: Res<RenderQueue>,
    mut last: Local<Option<([[f32; 4]; 3], f32)>>,
) {
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
