use std::num::NonZeroU64;

use wgpu::{
    BindGroupLayout, BindGroupLayoutEntry, BindingType, BufferBindingType, Device, ShaderStages,
};

use crate::entity_library::EntityLibrary;
use crate::frame_uniform::FrameUniform;

/// What a binding of the entity passes holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// The frame uniform.
    Frame,
    /// A static table that a uniform array holds: the models, the bones and the variants.
    Table,
    Storage {
        read_only: bool,
    },
    /// The depth pyramid of the cull.
    Pyramid,
}

const READ: Slot = Slot::Storage { read_only: true };
const WRITE: Slot = Slot::Storage { read_only: false };

/// The bindings of each compute pass, as its shader declares them. Each pass binds only what its
/// entry point reads, so that no stage binds more storage buffers than a WebGPU device has.
pub(crate) const SCATTER: [(u32, Slot); 4] = [(0, Slot::Frame), (1, WRITE), (3, WRITE), (17, READ)];
pub(crate) const CULL: [(u32, Slot); 9] = [
    (0, Slot::Frame),
    (4, READ),
    (5, READ),
    (8, Slot::Table),
    (11, WRITE),
    (15, WRITE),
    (18, Slot::Pyramid),
    (21, WRITE),
    (22, READ),
];
pub(crate) const POSE: [(u32, Slot); 11] = [
    (0, Slot::Frame),
    (1, WRITE),
    (3, WRITE),
    (5, READ),
    (6, READ),
    (8, Slot::Table),
    (9, Slot::Table),
    (11, WRITE),
    (12, WRITE),
    (13, WRITE),
    (15, WRITE),
];
pub(crate) const EXPAND: [(u32, Slot); 12] = [
    (0, Slot::Frame),
    (5, READ),
    (8, Slot::Table),
    (9, Slot::Table),
    (10, READ),
    (11, WRITE),
    (12, WRITE),
    (13, WRITE),
    (14, WRITE),
    (15, WRITE),
    (19, Slot::Table),
    (20, READ),
];
pub(crate) const PREPARE: [(u32, Slot); 9] = [
    (0, Slot::Frame),
    (1, WRITE),
    (3, WRITE),
    (5, READ),
    (15, WRITE),
    (21, WRITE),
    (22, READ),
    (24, WRITE),
    (25, READ),
];
pub(crate) const ARGS: [(u32, Slot); 3] = [(0, Slot::Frame), (15, WRITE), (16, WRITE)];
/// The bindings of the vertex stage of the draw; the frame uniform is in both stages.
pub(crate) const DRAW: [(u32, Slot); 9] = [
    (0, Slot::Frame),
    (5, READ),
    (8, Slot::Table),
    (10, READ),
    (11, READ),
    (12, READ),
    (13, READ),
    (14, READ),
    (19, Slot::Table),
];

const PASSES: [&[(u32, Slot)]; 7] = [&SCATTER, &CULL, &POSE, &EXPAND, &PREPARE, &ARGS, &DRAW];

/// Whether a device with `limits` binds every layout of the entity passes. A pipeline holds one of
/// them at a time, so the widest decides.
pub(crate) fn passes_fit(limits: &wgpu::Limits) -> bool {
    let fits = |count: usize, limit: u32| u32::try_from(count).is_ok_and(|count| count <= limit);
    PASSES.iter().all(|bindings| {
        let storage = bindings
            .iter()
            .filter(|(_, slot)| matches!(slot, Slot::Storage { .. }))
            .count();
        let uniform = bindings
            .iter()
            .filter(|(_, slot)| matches!(slot, Slot::Frame | Slot::Table))
            .count();
        fits(storage, limits.max_storage_buffers_per_shader_stage)
            && fits(uniform, limits.max_uniform_buffers_per_shader_stage)
    })
}

/// Whether the static tables of `library` fit the uniform arrays of a device with `limits`.
pub(crate) fn tables_fit(library: &EntityLibrary, limits: &wgpu::Limits) -> bool {
    let limit = limits.max_uniform_buffer_binding_size;
    [
        size_of_val(library.models.as_slice()),
        size_of_val(library.bones.as_slice()),
        size_of_val(library.variants.as_slice()),
    ]
    .into_iter()
    .all(|bytes| u64::try_from(bytes).is_ok_and(|bytes| bytes <= limit))
}

/// The bind group layouts of the entity passes.
pub(crate) struct Layouts {
    pub(crate) scatter: BindGroupLayout,
    pub(crate) cull: BindGroupLayout,
    pub(crate) pose: BindGroupLayout,
    pub(crate) expand: BindGroupLayout,
    /// The prepare pass of the impostors.
    pub(crate) prepare: BindGroupLayout,
    /// The passes that write the indirect arguments.
    pub(crate) args: BindGroupLayout,
    pub(crate) draw: BindGroupLayout,
    pub(crate) texture: BindGroupLayout,
    /// The draw of the impostors.
    pub(crate) impostor: BindGroupLayout,
    /// The colour and the normal images of the impostors.
    pub(crate) impostor_textures: BindGroupLayout,
}

impl Layouts {
    pub(crate) fn new(device: &Device) -> Self {
        let compute = |label: &str, bindings: &[(u32, Slot)]| {
            layout(device, label, bindings, |_| ShaderStages::COMPUTE)
        };
        let draw = layout(device, "entity draw", &DRAW, |slot| match slot {
            Slot::Frame => ShaderStages::VERTEX_FRAGMENT,
            _ => ShaderStages::VERTEX,
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("entity textures"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let (impostor_layout, impostor_textures) = impostor_layouts(device);
        Self {
            scatter: compute("entity scatter", &SCATTER),
            cull: compute("entity cull", &CULL),
            pose: compute("entity pose", &POSE),
            expand: compute("entity expand", &EXPAND),
            prepare: compute("entity impostor prepare", &PREPARE),
            args: compute("entity args", &ARGS),
            draw,
            texture: texture_layout,
            impostor: impostor_layout,
            impostor_textures,
        }
    }
}

/// The layout of `bindings`, each visible to the stages that `stages` gives it.
fn layout(
    device: &Device,
    label: &str,
    bindings: &[(u32, Slot)],
    stages: impl Fn(Slot) -> ShaderStages,
) -> BindGroupLayout {
    let entries: Vec<BindGroupLayoutEntry> = bindings
        .iter()
        .map(|&(binding, slot)| {
            let visibility = stages(slot);
            match slot {
                Slot::Frame => uniform_entry(binding, visibility),
                Slot::Table => BindGroupLayoutEntry {
                    binding,
                    visibility,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                Slot::Storage { read_only } => storage_entry(binding, read_only, visibility),
                Slot::Pyramid => BindGroupLayoutEntry {
                    binding,
                    visibility,
                    ty: BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            }
        })
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

/// The layouts of the impostor draw: its buffers, and its colour and normal images.
fn impostor_layouts(device: &Device) -> (BindGroupLayout, BindGroupLayout) {
    let both = ShaderStages::VERTEX_FRAGMENT;
    let impostor_entries = [uniform_entry(0, both), storage_entry(24, true, both)];
    let impostor_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("entity impostor"),
        entries: &impostor_entries,
    });
    let image_entry = |binding: u32| BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    };
    let impostor_textures = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("entity impostor images"),
        entries: &[image_entry(0), image_entry(1)],
    });
    (impostor_layout, impostor_textures)
}

fn storage_entry(binding: u32, read_only: bool, visibility: ShaderStages) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32, visibility: ShaderStages) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(size_of::<FrameUniform>() as u64),
        },
        count: None,
    }
}
