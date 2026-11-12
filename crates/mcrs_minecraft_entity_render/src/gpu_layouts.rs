use std::num::NonZeroU64;

use wgpu::{
    BindGroupLayout, BindGroupLayoutEntry, BindingType, BufferBindingType, Device, ShaderStages,
};

use crate::frame_uniform::FrameUniform;

/// The storage buffers of the compute layout by binding, each with whether it is read only.
const COMPUTE_STORAGE: [(u32, bool); 22] = [
    (1, false),
    (2, false),
    (3, false),
    (4, true),
    (5, true),
    (6, true),
    (8, true),
    (9, true),
    (10, true),
    (11, false),
    (12, false),
    (13, false),
    (14, false),
    (15, false),
    (17, true),
    (19, true),
    (20, true),
    (21, false),
    (22, true),
    (23, true),
    (24, false),
    (25, true),
];

/// The storage buffers the vertex stage of the draw reads, by binding.
const DRAW_STORAGE: [u32; 8] = [5, 8, 10, 11, 12, 13, 14, 19];

/// Whether a device with `limits` binds every layout of the entity passes. A pipeline holds one of
/// them at a time, so the widest decides.
pub(crate) fn passes_fit(limits: &wgpu::Limits) -> bool {
    let widest = COMPUTE_STORAGE.len().max(DRAW_STORAGE.len());
    usize::try_from(limits.max_storage_buffers_per_shader_stage).is_ok_and(|limit| limit >= widest)
}

/// The bind group layouts of the entity passes.
pub(crate) struct Layouts {
    /// The compute passes, with each buffer but the indirect arguments.
    pub(crate) compute: BindGroupLayout,
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
        let compute = ShaderStages::COMPUTE;
        let mut compute_entries = vec![uniform_entry(0, compute)];
        for (binding, read_only) in COMPUTE_STORAGE {
            compute_entries.push(storage_entry(binding, read_only, compute));
        }
        compute_entries.push(BindGroupLayoutEntry {
            binding: 18,
            visibility: compute,
            ty: BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("entity compute"),
            entries: &compute_entries,
        });
        let args_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("entity args"),
            entries: &[
                uniform_entry(0, compute),
                storage_entry(15, false, compute),
                storage_entry(16, false, compute),
            ],
        });
        let vertex = ShaderStages::VERTEX;
        let mut draw_entries = vec![uniform_entry(0, ShaderStages::VERTEX_FRAGMENT)];
        for binding in DRAW_STORAGE {
            draw_entries.push(storage_entry(binding, true, vertex));
        }
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("entity draw"),
            entries: &draw_entries,
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
            compute: compute_layout,
            args: args_layout,
            draw: draw_layout,
            texture: texture_layout,
            impostor: impostor_layout,
            impostor_textures,
        }
    }
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
