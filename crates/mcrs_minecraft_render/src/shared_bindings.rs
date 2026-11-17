use std::num::NonZeroU64;

use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_2d, texture_2d_array,
    texture_depth_2d, texture_storage_2d, uniform_buffer_sized,
};
use bevy::render::render_resource::{
    BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, BufferBinding, Sampler,
    SamplerBindingType, ShaderStages, ShaderType, StorageTextureAccess, TextureFormat,
    TextureSampleType, TextureView,
};
use bevy::render::view::ViewUniform;

use crate::render_path::RenderPath;
use crate::shared_types::{
    CAMERA_SIZE, PARAMS_SIZE, RECONSTRUCTION_SIZE, SKY_SIZE, TEXT_FRAME_SIZE,
};
use crate::words::Words;

pub mod terrain_view {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, BufferBinding,
        CAMERA_SIZE, NonZeroU64, PARAMS_SIZE, ShaderStages, uniform_buffer_sized,
    };

    pub const PARAMS: u32 = 0;
    pub const CAMERA: u32 = 1;

    pub struct Entries<'a> {
        pub params: &'a Buffer,
        pub camera: &'a Buffer,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "terrain frame",
            &[
                uniform_buffer_sized(true, NonZeroU64::new(PARAMS_SIZE)).build(
                    PARAMS,
                    ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                ),
                uniform_buffer_sized(false, NonZeroU64::new(CAMERA_SIZE)).build(
                    CAMERA,
                    ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                ),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 2] {
        [
            BindGroupEntry {
                binding: PARAMS,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: entries.params,
                    offset: 0,
                    size: NonZeroU64::new(PARAMS_SIZE),
                }),
            },
            BindGroupEntry {
                binding: CAMERA,
                resource: entries.camera.as_entire_binding(),
            },
        ]
    }
}

pub mod terrain_draw {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, RenderPath, Sampler,
        SamplerBindingType, ShaderStages, TextureSampleType, TextureView, Words, sampler,
        storage_buffer_read_only_sized, texture_2d, texture_2d_array,
    };

    pub const QUADS: u32 = 0;
    pub const VERTICES: u32 = 1;
    pub const VISIBLE: u32 = 2;
    pub const ATLAS0: u32 = 3;
    pub const ATLAS1: u32 = 4;
    pub const ATLAS2: u32 = 5;
    pub const ATLAS3: u32 = 6;
    pub const ATLAS_SAMPLER: u32 = 7;
    pub const TINTS: u32 = 8;
    pub const TINT_SAMPLER: u32 = 9;
    pub const ANIMATIONS: u32 = 10;
    pub const SPRITES: u32 = 11;
    pub const FACES: u32 = 12;
    pub const SECTIONS: u32 = 13;
    pub const LIGHTMAP: u32 = 14;
    pub const LIGHTMAP_SAMPLER: u32 = 15;

    pub struct Entries<'a> {
        pub quads: &'a Words,
        pub vertices: &'a Words,
        pub visible: Option<&'a Buffer>,
        pub atlas0: &'a TextureView,
        pub atlas1: &'a TextureView,
        pub atlas2: &'a TextureView,
        pub atlas3: &'a TextureView,
        pub atlas_sampler: &'a Sampler,
        pub tints: &'a TextureView,
        pub tint_sampler: &'a Sampler,
        pub animations: &'a Words,
        pub sprites: &'a Words,
        pub faces: &'a Words,
        pub sections: &'a Words,
        pub lightmap_levels: &'a TextureView,
        pub lightmap_sampler: &'a Sampler,
    }

    pub fn layout(path: RenderPath) -> BindGroupLayoutDescriptor {
        let words_layout = Words::layout(path);
        let mut entries = vec![
            words_layout.build(QUADS, ShaderStages::VERTEX),
            words_layout.build(VERTICES, ShaderStages::VERTEX),
            texture_2d_array(TextureSampleType::Float { filterable: true })
                .build(ATLAS0, ShaderStages::FRAGMENT),
            texture_2d_array(TextureSampleType::Float { filterable: true })
                .build(ATLAS1, ShaderStages::FRAGMENT),
            texture_2d_array(TextureSampleType::Float { filterable: true })
                .build(ATLAS2, ShaderStages::FRAGMENT),
            texture_2d_array(TextureSampleType::Float { filterable: true })
                .build(ATLAS3, ShaderStages::FRAGMENT),
            sampler(SamplerBindingType::Filtering).build(ATLAS_SAMPLER, ShaderStages::FRAGMENT),
            texture_2d_array(TextureSampleType::Float { filterable: true })
                .build(TINTS, ShaderStages::FRAGMENT),
            sampler(SamplerBindingType::Filtering).build(TINT_SAMPLER, ShaderStages::FRAGMENT),
            words_layout.build(ANIMATIONS, ShaderStages::FRAGMENT),
            words_layout.build(SPRITES, ShaderStages::FRAGMENT),
            words_layout.build(FACES, ShaderStages::FRAGMENT),
            words_layout.build(SECTIONS, ShaderStages::VERTEX),
            texture_2d(TextureSampleType::Float { filterable: true })
                .build(LIGHTMAP, ShaderStages::FRAGMENT),
            sampler(SamplerBindingType::Filtering).build(LIGHTMAP_SAMPLER, ShaderStages::FRAGMENT),
        ];
        if path.is_full() {
            entries.push(
                storage_buffer_read_only_sized(false, None).build(VISIBLE, ShaderStages::VERTEX),
            );
        }
        BindGroupLayoutDescriptor::new("terrain draw data", &entries)
    }

    pub fn entries<'a>(entries: Entries<'a>) -> Vec<BindGroupEntry<'a>> {
        let mut out = vec![
            BindGroupEntry {
                binding: QUADS,
                resource: entries.quads.binding(),
            },
            BindGroupEntry {
                binding: VERTICES,
                resource: entries.vertices.binding(),
            },
            BindGroupEntry {
                binding: ATLAS0,
                resource: BindingResource::TextureView(entries.atlas0),
            },
            BindGroupEntry {
                binding: ATLAS1,
                resource: BindingResource::TextureView(entries.atlas1),
            },
            BindGroupEntry {
                binding: ATLAS2,
                resource: BindingResource::TextureView(entries.atlas2),
            },
            BindGroupEntry {
                binding: ATLAS3,
                resource: BindingResource::TextureView(entries.atlas3),
            },
            BindGroupEntry {
                binding: ATLAS_SAMPLER,
                resource: BindingResource::Sampler(entries.atlas_sampler),
            },
            BindGroupEntry {
                binding: TINTS,
                resource: BindingResource::TextureView(entries.tints),
            },
            BindGroupEntry {
                binding: TINT_SAMPLER,
                resource: BindingResource::Sampler(entries.tint_sampler),
            },
            BindGroupEntry {
                binding: ANIMATIONS,
                resource: entries.animations.binding(),
            },
            BindGroupEntry {
                binding: SPRITES,
                resource: entries.sprites.binding(),
            },
            BindGroupEntry {
                binding: FACES,
                resource: entries.faces.binding(),
            },
            BindGroupEntry {
                binding: SECTIONS,
                resource: entries.sections.binding(),
            },
            BindGroupEntry {
                binding: LIGHTMAP,
                resource: BindingResource::TextureView(entries.lightmap_levels),
            },
            BindGroupEntry {
                binding: LIGHTMAP_SAMPLER,
                resource: BindingResource::Sampler(entries.lightmap_sampler),
            },
        ];
        if let Some(buf) = entries.visible {
            out.push(BindGroupEntry {
                binding: VISIBLE,
                resource: buf.as_entire_binding(),
            });
        }
        out
    }
}

pub mod cull {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, ShaderStages,
        TextureSampleType, TextureView, Words, storage_buffer_read_only_sized,
        storage_buffer_sized, texture_2d,
    };

    pub const GROUPS: u32 = 0;
    pub const VISIBLE: u32 = 1;
    pub const ARGS: u32 = 2;
    pub const CAVE_VISIBLE: u32 = 3;
    pub const SECTIONS: u32 = 4;
    pub const BATCHES: u32 = 5;
    pub const HIZ: u32 = 6;
    pub const CANDIDATES: u32 = 7;
    pub const QUAD_WORDS: u32 = 8;
    pub const MODEL_WORDS: u32 = 9;

    pub struct OrderedEntries<'a> {
        pub groups: &'a Words,
        pub visible: &'a Buffer,
        pub args: &'a Buffer,
        pub cave_visible: &'a Buffer,
        pub sections: &'a Words,
        pub batches: &'a Words,
        pub hiz: &'a TextureView,
        pub candidates: &'a Words,
    }

    pub fn ordered_layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "terrain cull data",
            &[
                storage_buffer_read_only_sized(false, None).build(GROUPS, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(VISIBLE, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(ARGS, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(CAVE_VISIBLE, ShaderStages::COMPUTE),
                storage_buffer_read_only_sized(false, None).build(SECTIONS, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(BATCHES, ShaderStages::COMPUTE),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(HIZ, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(CANDIDATES, ShaderStages::COMPUTE),
            ],
        )
    }

    pub fn ordered_entries<'a>(entries: OrderedEntries<'a>) -> [BindGroupEntry<'a>; 8] {
        [
            BindGroupEntry {
                binding: GROUPS,
                resource: entries.groups.binding(),
            },
            BindGroupEntry {
                binding: VISIBLE,
                resource: entries.visible.as_entire_binding(),
            },
            BindGroupEntry {
                binding: ARGS,
                resource: entries.args.as_entire_binding(),
            },
            BindGroupEntry {
                binding: CAVE_VISIBLE,
                resource: entries.cave_visible.as_entire_binding(),
            },
            BindGroupEntry {
                binding: SECTIONS,
                resource: entries.sections.binding(),
            },
            BindGroupEntry {
                binding: BATCHES,
                resource: entries.batches.binding(),
            },
            BindGroupEntry {
                binding: HIZ,
                resource: BindingResource::TextureView(entries.hiz),
            },
            BindGroupEntry {
                binding: CANDIDATES,
                resource: entries.candidates.binding(),
            },
        ]
    }

    pub struct QuadEntries<'a> {
        pub groups: &'a Words,
        pub visible: &'a Buffer,
        pub args: &'a Buffer,
        pub cave_visible: &'a Buffer,
        pub sections: &'a Words,
        pub hiz: &'a TextureView,
        pub candidates: &'a Words,
        pub quad_words: &'a Words,
        pub model_words: &'a Words,
    }

    pub fn quad_layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "terrain quad cull data",
            &[
                storage_buffer_read_only_sized(false, None).build(GROUPS, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(VISIBLE, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(ARGS, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(CAVE_VISIBLE, ShaderStages::COMPUTE),
                storage_buffer_read_only_sized(false, None).build(SECTIONS, ShaderStages::COMPUTE),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(HIZ, ShaderStages::COMPUTE),
                storage_buffer_sized(false, None).build(CANDIDATES, ShaderStages::COMPUTE),
                storage_buffer_read_only_sized(false, None)
                    .build(QUAD_WORDS, ShaderStages::COMPUTE),
                storage_buffer_read_only_sized(false, None)
                    .build(MODEL_WORDS, ShaderStages::COMPUTE),
            ],
        )
    }

    pub fn quad_entries<'a>(entries: QuadEntries<'a>) -> [BindGroupEntry<'a>; 9] {
        [
            BindGroupEntry {
                binding: GROUPS,
                resource: entries.groups.binding(),
            },
            BindGroupEntry {
                binding: VISIBLE,
                resource: entries.visible.as_entire_binding(),
            },
            BindGroupEntry {
                binding: ARGS,
                resource: entries.args.as_entire_binding(),
            },
            BindGroupEntry {
                binding: CAVE_VISIBLE,
                resource: entries.cave_visible.as_entire_binding(),
            },
            BindGroupEntry {
                binding: SECTIONS,
                resource: entries.sections.binding(),
            },
            BindGroupEntry {
                binding: HIZ,
                resource: BindingResource::TextureView(entries.hiz),
            },
            BindGroupEntry {
                binding: CANDIDATES,
                resource: entries.candidates.binding(),
            },
            BindGroupEntry {
                binding: QUAD_WORDS,
                resource: entries.quad_words.binding(),
            },
            BindGroupEntry {
                binding: MODEL_WORDS,
                resource: entries.model_words.binding(),
            },
        ]
    }
}

pub mod gbuffer {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, NonZeroU64,
        RECONSTRUCTION_SIZE, Sampler, SamplerBindingType, ShaderStages, TextureSampleType,
        TextureView, sampler, texture_2d, uniform_buffer_sized,
    };

    pub const ALBEDO_AO: u32 = 0;
    pub const NORMAL_MOTION: u32 = 1;
    pub const LIGHT: u32 = 2;
    pub const DEPTH: u32 = 3;
    pub const RECONSTRUCTION: u32 = 4;
    pub const NEAREST: u32 = 5;

    pub struct Entries<'a> {
        pub albedo_ao: &'a TextureView,
        pub normal_motion: &'a TextureView,
        pub light: &'a TextureView,
        pub depth: &'a TextureView,
        pub reconstruction: &'a Buffer,
        pub nearest: &'a Sampler,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "deferred gbuffer",
            &[
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(ALBEDO_AO, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(NORMAL_MOTION, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(LIGHT, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(DEPTH, ShaderStages::FRAGMENT),
                uniform_buffer_sized(false, NonZeroU64::new(RECONSTRUCTION_SIZE))
                    .build(RECONSTRUCTION, ShaderStages::FRAGMENT),
                sampler(SamplerBindingType::NonFiltering).build(NEAREST, ShaderStages::FRAGMENT),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 6] {
        [
            BindGroupEntry {
                binding: ALBEDO_AO,
                resource: BindingResource::TextureView(entries.albedo_ao),
            },
            BindGroupEntry {
                binding: NORMAL_MOTION,
                resource: BindingResource::TextureView(entries.normal_motion),
            },
            BindGroupEntry {
                binding: LIGHT,
                resource: BindingResource::TextureView(entries.light),
            },
            BindGroupEntry {
                binding: DEPTH,
                resource: BindingResource::TextureView(entries.depth),
            },
            BindGroupEntry {
                binding: RECONSTRUCTION,
                resource: entries.reconstruction.as_entire_binding(),
            },
            BindGroupEntry {
                binding: NEAREST,
                resource: BindingResource::Sampler(entries.nearest),
            },
        ]
    }
}

pub(crate) mod transparency {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Sampler, SamplerBindingType,
        ShaderStages, TextureSampleType, TextureView, sampler, texture_2d,
    };

    pub const SKY_IMAGE: u32 = 6;
    pub const CLOUD_IMAGE: u32 = 7;
    pub const CLOUD_DEPTH: u32 = 8;
    pub const CLOUD_NEAREST: u32 = 9;

    pub struct Entries<'a> {
        pub sky_image: &'a TextureView,
        pub cloud_image: &'a TextureView,
        pub cloud_depth: &'a TextureView,
        pub cloud_nearest: &'a Sampler,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "improved transparency",
            &[
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(SKY_IMAGE, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(CLOUD_IMAGE, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(CLOUD_DEPTH, ShaderStages::FRAGMENT),
                sampler(SamplerBindingType::NonFiltering)
                    .build(CLOUD_NEAREST, ShaderStages::FRAGMENT),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 4] {
        [
            BindGroupEntry {
                binding: SKY_IMAGE,
                resource: BindingResource::TextureView(entries.sky_image),
            },
            BindGroupEntry {
                binding: CLOUD_IMAGE,
                resource: BindingResource::TextureView(entries.cloud_image),
            },
            BindGroupEntry {
                binding: CLOUD_DEPTH,
                resource: BindingResource::TextureView(entries.cloud_depth),
            },
            BindGroupEntry {
                binding: CLOUD_NEAREST,
                resource: BindingResource::Sampler(entries.cloud_nearest),
            },
        ]
    }
}

pub(crate) mod transparent_gbuffer {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, NonZeroU64,
        RECONSTRUCTION_SIZE, Sampler, SamplerBindingType, ShaderStages, TextureSampleType,
        TextureView, sampler, texture_2d, uniform_buffer_sized,
    };

    pub const ALBEDO_AO: u32 = 0;
    pub const NORMAL_MOTION: u32 = 1;
    pub const LIGHT: u32 = 2;
    pub const DEPTH: u32 = 3;
    pub const RECONSTRUCTION: u32 = 4;
    pub const NEAREST: u32 = 5;
    pub const SKY_IMAGE: u32 = 6;
    pub const CLOUD_IMAGE: u32 = 7;
    pub const CLOUD_DEPTH: u32 = 8;
    pub const CLOUD_NEAREST: u32 = 9;

    pub struct Entries<'a> {
        pub albedo_ao: &'a TextureView,
        pub normal_motion: &'a TextureView,
        pub light: &'a TextureView,
        pub depth: &'a TextureView,
        pub reconstruction: &'a Buffer,
        pub nearest: &'a Sampler,
        pub sky_image: &'a TextureView,
        pub cloud_image: &'a TextureView,
        pub cloud_depth: &'a TextureView,
        pub cloud_nearest: &'a Sampler,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "deferred gbuffer with improved transparency",
            &[
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(ALBEDO_AO, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(NORMAL_MOTION, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(LIGHT, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(DEPTH, ShaderStages::FRAGMENT),
                uniform_buffer_sized(false, NonZeroU64::new(RECONSTRUCTION_SIZE))
                    .build(RECONSTRUCTION, ShaderStages::FRAGMENT),
                sampler(SamplerBindingType::NonFiltering).build(NEAREST, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(SKY_IMAGE, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(CLOUD_IMAGE, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(CLOUD_DEPTH, ShaderStages::FRAGMENT),
                sampler(SamplerBindingType::NonFiltering)
                    .build(CLOUD_NEAREST, ShaderStages::FRAGMENT),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 10] {
        [
            BindGroupEntry {
                binding: ALBEDO_AO,
                resource: BindingResource::TextureView(entries.albedo_ao),
            },
            BindGroupEntry {
                binding: NORMAL_MOTION,
                resource: BindingResource::TextureView(entries.normal_motion),
            },
            BindGroupEntry {
                binding: LIGHT,
                resource: BindingResource::TextureView(entries.light),
            },
            BindGroupEntry {
                binding: DEPTH,
                resource: BindingResource::TextureView(entries.depth),
            },
            BindGroupEntry {
                binding: RECONSTRUCTION,
                resource: entries.reconstruction.as_entire_binding(),
            },
            BindGroupEntry {
                binding: NEAREST,
                resource: BindingResource::Sampler(entries.nearest),
            },
            BindGroupEntry {
                binding: SKY_IMAGE,
                resource: BindingResource::TextureView(entries.sky_image),
            },
            BindGroupEntry {
                binding: CLOUD_IMAGE,
                resource: BindingResource::TextureView(entries.cloud_image),
            },
            BindGroupEntry {
                binding: CLOUD_DEPTH,
                resource: BindingResource::TextureView(entries.cloud_depth),
            },
            BindGroupEntry {
                binding: CLOUD_NEAREST,
                resource: BindingResource::Sampler(entries.cloud_nearest),
            },
        ]
    }
}

pub(crate) mod sky_view {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, BufferBinding,
        NonZeroU64, SKY_SIZE, ShaderStages, ShaderType, ViewUniform, uniform_buffer_sized,
    };

    pub const VIEW: u32 = 0;
    pub const SKY: u32 = 1;

    pub struct Entries<'a> {
        pub view: &'a Buffer,
        pub sky: &'a Buffer,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "sky view",
            &[
                uniform_buffer_sized(true, Some(ViewUniform::min_size()))
                    .build(VIEW, ShaderStages::VERTEX_FRAGMENT),
                uniform_buffer_sized(false, NonZeroU64::new(SKY_SIZE))
                    .build(SKY, ShaderStages::VERTEX_FRAGMENT),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 2] {
        [
            BindGroupEntry {
                binding: VIEW,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: entries.view,
                    offset: 0,
                    size: Some(ViewUniform::min_size()),
                }),
            },
            BindGroupEntry {
                binding: SKY,
                resource: entries.sky.as_entire_binding(),
            },
        ]
    }
}

pub(crate) mod sky_textures {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Sampler, SamplerBindingType,
        ShaderStages, TextureSampleType, TextureView, sampler, texture_2d, texture_2d_array,
    };

    pub const CELESTIALS: u32 = 0;
    pub const CELESTIAL_SAMPLER: u32 = 1;
    pub const CLOUDS: u32 = 2;

    pub struct Entries<'a> {
        pub celestials: &'a TextureView,
        pub celestial_sampler: &'a Sampler,
        pub clouds: &'a TextureView,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "sky textures",
            &[
                texture_2d_array(TextureSampleType::Float { filterable: true })
                    .build(CELESTIALS, ShaderStages::VERTEX_FRAGMENT),
                sampler(SamplerBindingType::Filtering)
                    .build(CELESTIAL_SAMPLER, ShaderStages::VERTEX_FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: true })
                    .build(CLOUDS, ShaderStages::VERTEX_FRAGMENT),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 3] {
        [
            BindGroupEntry {
                binding: CELESTIALS,
                resource: BindingResource::TextureView(entries.celestials),
            },
            BindGroupEntry {
                binding: CELESTIAL_SAMPLER,
                resource: BindingResource::Sampler(entries.celestial_sampler),
            },
            BindGroupEntry {
                binding: CLOUDS,
                resource: BindingResource::TextureView(entries.clouds),
            },
        ]
    }
}

pub mod hiz {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, ShaderStages,
        StorageTextureAccess, TextureFormat, TextureSampleType, TextureView, texture_2d,
        texture_depth_2d, texture_storage_2d,
    };

    pub const DEPTH: u32 = 0;
    pub const LEVEL_OUT: u32 = 1;
    pub const LEVEL_IN: u32 = 2;

    pub struct FromDepthEntries<'a> {
        pub depth: &'a TextureView,
        pub level_out: &'a TextureView,
    }

    pub fn from_depth_layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "hiz from depth",
            &[
                texture_depth_2d().build(DEPTH, ShaderStages::COMPUTE),
                texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly)
                    .build(LEVEL_OUT, ShaderStages::COMPUTE),
            ],
        )
    }

    pub fn from_depth_entries<'a>(entries: FromDepthEntries<'a>) -> [BindGroupEntry<'a>; 2] {
        [
            BindGroupEntry {
                binding: DEPTH,
                resource: BindingResource::TextureView(entries.depth),
            },
            BindGroupEntry {
                binding: LEVEL_OUT,
                resource: BindingResource::TextureView(entries.level_out),
            },
        ]
    }

    pub struct DownEntries<'a> {
        pub level_out: &'a TextureView,
        pub level_in: &'a TextureView,
    }

    pub fn down_layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "hiz down",
            &[
                texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly)
                    .build(LEVEL_OUT, ShaderStages::COMPUTE),
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(LEVEL_IN, ShaderStages::COMPUTE),
            ],
        )
    }

    pub fn down_entries<'a>(entries: DownEntries<'a>) -> [BindGroupEntry<'a>; 2] {
        [
            BindGroupEntry {
                binding: LEVEL_OUT,
                resource: BindingResource::TextureView(entries.level_out),
            },
            BindGroupEntry {
                binding: LEVEL_IN,
                resource: BindingResource::TextureView(entries.level_in),
            },
        ]
    }
}

pub mod present {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, ShaderStages,
        TextureSampleType, TextureView, texture_2d,
    };

    pub const WORLD: u32 = 0;

    pub struct Entries<'a> {
        pub world: &'a TextureView,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "present",
            &[texture_2d(TextureSampleType::Float { filterable: false })
                .build(WORLD, ShaderStages::FRAGMENT)],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 1] {
        [BindGroupEntry {
            binding: WORLD,
            resource: BindingResource::TextureView(entries.world),
        }]
    }
}

pub(crate) mod show {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Sampler, SamplerBindingType,
        ShaderStages, TextureSampleType, TextureView, sampler, texture_2d,
    };

    pub const SOURCE: u32 = 0;
    pub const NEAREST: u32 = 1;

    pub struct Entries<'a> {
        pub source: &'a TextureView,
        pub nearest: &'a Sampler,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "show depth",
            &[
                texture_2d(TextureSampleType::Float { filterable: false })
                    .build(SOURCE, ShaderStages::FRAGMENT),
                sampler(SamplerBindingType::NonFiltering).build(NEAREST, ShaderStages::FRAGMENT),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 2] {
        [
            BindGroupEntry {
                binding: SOURCE,
                resource: BindingResource::TextureView(entries.source),
            },
            BindGroupEntry {
                binding: NEAREST,
                resource: BindingResource::Sampler(entries.nearest),
            },
        ]
    }
}

pub mod world_text_view {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, Buffer, NonZeroU64, ShaderStages,
        TEXT_FRAME_SIZE, uniform_buffer_sized,
    };

    pub const FRAME: u32 = 0;

    pub struct Entries<'a> {
        pub frame: &'a Buffer,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "world text view",
            &[
                uniform_buffer_sized(false, NonZeroU64::new(TEXT_FRAME_SIZE))
                    .build(FRAME, ShaderStages::VERTEX),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 1] {
        [BindGroupEntry {
            binding: FRAME,
            resource: entries.frame.as_entire_binding(),
        }]
    }
}

pub mod world_text_textures {
    use super::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Sampler, SamplerBindingType,
        ShaderStages, TextureSampleType, TextureView, sampler, texture_2d,
    };

    pub const FONT_PAGE: u32 = 0;
    pub const FONT_SAMPLER: u32 = 1;
    pub const LIGHTMAP_LEVELS: u32 = 2;
    pub const LIGHTMAP_SAMPLER: u32 = 3;

    pub struct Entries<'a> {
        pub font_page: &'a TextureView,
        pub font_sampler: &'a Sampler,
        pub lightmap_levels: &'a TextureView,
        pub lightmap_sampler: &'a Sampler,
    }

    pub fn layout() -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "world text texture",
            &[
                texture_2d(TextureSampleType::Float { filterable: true })
                    .build(FONT_PAGE, ShaderStages::FRAGMENT),
                sampler(SamplerBindingType::Filtering).build(FONT_SAMPLER, ShaderStages::FRAGMENT),
                texture_2d(TextureSampleType::Float { filterable: true })
                    .build(LIGHTMAP_LEVELS, ShaderStages::FRAGMENT),
                sampler(SamplerBindingType::Filtering)
                    .build(LIGHTMAP_SAMPLER, ShaderStages::FRAGMENT),
            ],
        )
    }

    pub fn entries<'a>(entries: Entries<'a>) -> [BindGroupEntry<'a>; 4] {
        [
            BindGroupEntry {
                binding: FONT_PAGE,
                resource: BindingResource::TextureView(entries.font_page),
            },
            BindGroupEntry {
                binding: FONT_SAMPLER,
                resource: BindingResource::Sampler(entries.font_sampler),
            },
            BindGroupEntry {
                binding: LIGHTMAP_LEVELS,
                resource: BindingResource::TextureView(entries.lightmap_levels),
            },
            BindGroupEntry {
                binding: LIGHTMAP_SAMPLER,
                resource: BindingResource::Sampler(entries.lightmap_sampler),
            },
        ]
    }
}
