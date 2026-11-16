use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use mcrs_minecraft_mesh::pack::MAX_SPRITE_ARRAYS;

use super::arenas::Arenas;
use super::cull::Cull;
use super::frame::Frame;
use super::render_path::RenderPath;
use super::shared_bindings::{terrain_draw, terrain_view};
use super::sprites::Sprites;

const _: () = assert!(MAX_SPRITE_ARRAYS == 4);

pub(super) struct Bindings {
    pub view_layout: BindGroupLayoutDescriptor,
    pub draw_layout: BindGroupLayoutDescriptor,
    pub view: BindGroup,
    pub draw: BindGroup,
}

impl Bindings {
    pub fn new(
        path: RenderPath,
        frame: &Frame,
        arenas: &Arenas,
        cull: Option<&Cull>,
        sprites: &Sprites,
        device: &RenderDevice,
        pipeline_cache: &PipelineCache,
    ) -> Self {
        let view_layout = terrain_view::layout();
        let draw_layout = terrain_draw::layout(path);
        let view = device.create_bind_group(
            "terrain view",
            &pipeline_cache.get_bind_group_layout(&view_layout),
            &terrain_view::entries(terrain_view::Entries {
                params: &frame.params,
                camera: &frame.camera,
            }),
        );
        let draw = draw_bind_group(&draw_layout, arenas, cull, sprites, device, pipeline_cache);
        Self {
            view_layout,
            draw_layout,
            view,
            draw,
        }
    }

    pub fn rebuild_draw(
        &mut self,
        arenas: &Arenas,
        cull: Option<&Cull>,
        sprites: &Sprites,
        device: &RenderDevice,
        pipeline_cache: &PipelineCache,
    ) {
        self.draw = draw_bind_group(
            &self.draw_layout,
            arenas,
            cull,
            sprites,
            device,
            pipeline_cache,
        );
    }
}

fn draw_bind_group(
    layout: &BindGroupLayoutDescriptor,
    arenas: &Arenas,
    cull: Option<&Cull>,
    sprites: &Sprites,
    device: &RenderDevice,
    pipeline_cache: &PipelineCache,
) -> BindGroup {
    let [atlas0, atlas1, atlas2, atlas3] = &sprites.atlases;
    let entries = terrain_draw::entries(terrain_draw::Entries {
        quads: &arenas.quads.words,
        vertices: &arenas.vertices.words,
        visible: cull.map(Cull::visible),
        atlas0: &atlas0.view,
        atlas1: &atlas1.view,
        atlas2: &atlas2.view,
        atlas3: &atlas3.view,
        atlas_sampler: &sprites.atlas_sampler,
        tints: &sprites.tints_view,
        tint_sampler: &sprites.tint_sampler,
        animations: &sprites.frames,
        sprites: &sprites.table,
        faces: &arenas.faces.words,
        sections: &arenas.sections,
        lightmap_levels: &sprites.lightmap_view,
        lightmap_sampler: &sprites.lightmap_sampler,
    });
    device.create_bind_group(
        "terrain draw",
        &pipeline_cache.get_bind_group_layout(layout),
        &entries,
    )
}
