use std::num::NonZeroU64;

use bevy::render::globals::GlobalsUniform;
use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_2d_array,
    uniform_buffer, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewUniform;

use crate::pack::MAX_SPRITE_ARRAYS;
use crate::sky::SkyUniform;

use super::arenas::Arenas;
use super::draws::PARAMS_SIZE;
use super::frame::Frame;
use super::sprites::Sprites;
use super::texture::array_view;

pub(super) struct Bindings {
    pub view_layout: BindGroupLayoutDescriptor,
    pub cull_layout: BindGroupLayoutDescriptor,
    pub draw_layout: BindGroupLayoutDescriptor,
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
        let cull = device.create_bind_group(
            "terrain cull",
            &pipeline_cache.get_bind_group_layout(&cull_layout),
            &BindGroupEntries::sequential((
                arenas.groups.as_entire_buffer_binding(),
                arenas.visible.as_entire_buffer_binding(),
                frame.args.as_entire_buffer_binding(),
                frame.cave.as_entire_buffer_binding(),
                arenas.sections.as_entire_buffer_binding(),
            )),
        );
        let draw = draw_bind_group(&draw_layout, arenas, sprites, device, pipeline_cache);
        Self {
            view_layout,
            cull_layout,
            draw_layout,
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
}

fn view_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "terrain view",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            (
                uniform_buffer_sized(true, Some(ViewUniform::min_size())),
                uniform_buffer_sized(true, NonZeroU64::new(PARAMS_SIZE)),
                uniform_buffer::<GlobalsUniform>(false),
                uniform_buffer_sized(false, NonZeroU64::new(size_of::<SkyUniform>() as u64)),
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
            ),
        ),
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
            &array_view(&sprites.tints),
            &sprites.tint_sampler,
            sprites.animations.as_entire_buffer_binding(),
            arenas.faces.as_entire_buffer_binding(),
            arenas.sections.as_entire_buffer_binding(),
        )),
    )
}
