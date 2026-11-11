use std::num::NonZeroU64;

use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, texture_2d, texture_2d_array, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use mcrs_minecraft_mesh::pack::MAX_SPRITE_ARRAYS;

use super::arenas::Arenas;
use super::cull::Cull;
use super::draws::PARAMS_SIZE;
use super::frame::{CAMERA_SIZE, Frame};
use super::render_path::RenderPath;
use super::sprites::Sprites;
use super::words::Words;

pub(super) struct Bindings {
    pub view_layout: BindGroupLayoutDescriptor,
    pub draw_layout: BindGroupLayoutDescriptor,
    pub view: BindGroup,
    pub draw: BindGroup,
}

/// The binding numbers of `terrain_bindings.wesl`.
const QUADS: u32 = 0;
const VERTICES: u32 = 1;
const VISIBLE: u32 = 2;
const ATLASES: u32 = 3;
const ATLAS_SAMPLER: u32 = 7;
const TINTS: u32 = 8;
const TINT_SAMPLER: u32 = 9;
const ANIMATIONS: u32 = 10;
const SPRITES: u32 = 11;
const FACES: u32 = 12;
const SECTIONS: u32 = 13;
const LIGHTMAP: u32 = 14;
const LIGHTMAP_SAMPLER: u32 = 16;

const _: () = assert!(MAX_SPRITE_ARRAYS == (ATLAS_SAMPLER - ATLASES) as usize);

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
        let view_layout = view_layout();
        let draw_layout = draw_layout(path);
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

pub(super) fn view_layout() -> BindGroupLayoutDescriptor {
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

/// Each binding is visible to the one stage that reads it: WebGL2 counts the textures of a
/// stage against sixteen, and on the minimal path the arrays of words are textures too.
fn draw_layout(path: RenderPath) -> BindGroupLayoutDescriptor {
    let (vertex, fragment) = (ShaderStages::VERTEX, ShaderStages::FRAGMENT);
    let words = Words::layout(path);
    let atlas = texture_2d_array(TextureSampleType::Float { filterable: true });
    let filtering = sampler(SamplerBindingType::Filtering);
    let mut entries = vec![
        words.build(QUADS, vertex),
        words.build(VERTICES, vertex),
        atlas.build(TINTS, fragment),
        filtering.build(ATLAS_SAMPLER, fragment),
        filtering.build(TINT_SAMPLER, fragment),
        words.build(ANIMATIONS, fragment),
        words.build(SPRITES, fragment),
        words.build(FACES, fragment),
        words.build(SECTIONS, vertex),
        texture_2d(TextureSampleType::Float { filterable: true }).build(LIGHTMAP, fragment),
        filtering.build(LIGHTMAP_SAMPLER, fragment),
    ];
    entries.extend((ATLASES..ATLAS_SAMPLER).map(|binding| atlas.build(binding, fragment)));
    if path.is_full() {
        entries.push(storage_buffer_read_only_sized(false, None).build(VISIBLE, vertex));
    }
    BindGroupLayoutDescriptor::new("terrain draw data", &entries)
}

fn draw_bind_group(
    layout: &BindGroupLayoutDescriptor,
    arenas: &Arenas,
    cull: Option<&Cull>,
    sprites: &Sprites,
    device: &RenderDevice,
    pipeline_cache: &PipelineCache,
) -> BindGroup {
    let entry = |binding, resource| BindGroupEntry { binding, resource };
    let mut entries = vec![
        entry(QUADS, arenas.quads.words.binding()),
        entry(VERTICES, arenas.vertices.words.binding()),
        entry(ATLAS_SAMPLER, sprites.atlas_sampler.into_binding()),
        entry(TINTS, sprites.tints_view.into_binding()),
        entry(TINT_SAMPLER, sprites.tint_sampler.into_binding()),
        entry(ANIMATIONS, sprites.frames.binding()),
        entry(SPRITES, sprites.table.binding()),
        entry(FACES, arenas.faces.words.binding()),
        entry(SECTIONS, arenas.sections.binding()),
        entry(LIGHTMAP, sprites.lightmap_view.into_binding()),
        entry(LIGHTMAP_SAMPLER, sprites.lightmap_sampler.into_binding()),
    ];
    entries.extend(
        (ATLASES..)
            .zip(&sprites.atlases)
            .map(|(binding, atlas)| entry(binding, atlas.view.into_binding())),
    );
    if let Some(cull) = cull {
        entries.push(entry(VISIBLE, cull.visible().as_entire_binding()));
    }
    device.create_bind_group(
        "terrain draw",
        &pipeline_cache.get_bind_group_layout(layout),
        &entries,
    )
}
