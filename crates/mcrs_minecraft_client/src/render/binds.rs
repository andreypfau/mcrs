use std::num::NonZeroU64;

use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_2d, texture_2d_array,
    uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use crate::pack::MAX_SPRITE_ARRAYS;

use super::arenas::Arenas;
use super::draws::PARAMS_SIZE;
use super::frame::{CAMERA_SIZE, Frame};
use super::sprites::Sprites;

pub(super) struct Bindings {
    pub view_layout: BindGroupLayoutDescriptor,
    pub cull_layout: BindGroupLayoutDescriptor,
    pub draw_layout: BindGroupLayoutDescriptor,
    pub view: BindGroup,
    pub cull: BindGroup,
    pub draw: BindGroup,
}

impl Bindings {
    pub fn new(
        arenas: &Arenas,
        frame: &Frame,
        sprites: &Sprites,
        device: &RenderDevice,
        pipeline_cache: &PipelineCache,
    ) -> Self {
        let view_layout = view_layout();
        let cull_layout = cull_layout();
        let draw_layout = draw_layout();
        let view = device.create_bind_group(
            "terrain view",
            &pipeline_cache.get_bind_group_layout(&view_layout),
            &BindGroupEntries::sequential((
                BufferBinding {
                    buffer: &frame.params,
                    offset: 0,
                    size: NonZeroU64::new(PARAMS_SIZE),
                },
                frame.camera.as_entire_buffer_binding(),
            )),
        );
        let cull = cull_bind_group(&cull_layout, arenas, frame, device, pipeline_cache);
        let draw = draw_bind_group(&draw_layout, arenas, sprites, device, pipeline_cache);
        Self {
            view_layout,
            cull_layout,
            draw_layout,
            view,
            cull,
            draw,
        }
    }

    pub fn rebuild_draw(
        &mut self,
        arenas: &Arenas,
        sprites: &Sprites,
        device: &RenderDevice,
        pipeline_cache: &PipelineCache,
    ) {
        self.draw = draw_bind_group(&self.draw_layout, arenas, sprites, device, pipeline_cache);
    }

    pub fn rebuild_cull(
        &mut self,
        arenas: &Arenas,
        frame: &Frame,
        device: &RenderDevice,
        pipeline_cache: &PipelineCache,
    ) {
        self.cull = cull_bind_group(&self.cull_layout, arenas, frame, device, pipeline_cache);
    }
}

fn view_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "terrain view",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            (
                uniform_buffer_sized(true, NonZeroU64::new(PARAMS_SIZE)),
                uniform_buffer_sized(false, NonZeroU64::new(CAMERA_SIZE)),
            ),
        ),
    )
}

fn cull_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "terrain cull data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
            ),
        ),
    )
}

fn draw_layout() -> BindGroupLayoutDescriptor {
    const _: () = assert!(MAX_SPRITE_ARRAYS == 4);
    BindGroupLayoutDescriptor::new(
        "terrain draw data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    )
}

fn cull_bind_group(
    layout: &BindGroupLayoutDescriptor,
    arenas: &Arenas,
    frame: &Frame,
    device: &RenderDevice,
    pipeline_cache: &PipelineCache,
) -> BindGroup {
    device.create_bind_group(
        "terrain cull",
        &pipeline_cache.get_bind_group_layout(layout),
        &BindGroupEntries::sequential((
            arenas.groups.as_entire_buffer_binding(),
            arenas.visible.as_entire_buffer_binding(),
            frame.args.as_entire_buffer_binding(),
            frame.cave.as_entire_buffer_binding(),
            arenas.sections.as_entire_buffer_binding(),
        )),
    )
}

fn draw_bind_group(
    layout: &BindGroupLayoutDescriptor,
    arenas: &Arenas,
    sprites: &Sprites,
    device: &RenderDevice,
    pipeline_cache: &PipelineCache,
) -> BindGroup {
    device.create_bind_group(
        "terrain draw",
        &pipeline_cache.get_bind_group_layout(layout),
        &BindGroupEntries::sequential((
            arenas.quads.as_entire_buffer_binding(),
            arenas.vertices.as_entire_buffer_binding(),
            arenas.visible.as_entire_buffer_binding(),
            &sprites.atlases[0],
            &sprites.atlases[1],
            &sprites.atlases[2],
            &sprites.atlases[3],
            &sprites.atlas_sampler,
            &sprites.tints_view,
            &sprites.tint_sampler,
            sprites.frames.as_entire_buffer_binding(),
            arenas.faces.as_entire_buffer_binding(),
            arenas.sections.as_entire_buffer_binding(),
            &sprites.lightmap_view,
        )),
    )
}
