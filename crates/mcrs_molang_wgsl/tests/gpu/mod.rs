//! A headless wgpu device for the GPU tests and the benchmark of this crate.

use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferUsages, ComputePipeline,
    ComputePipelineDescriptor, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor,
    PowerPreference, Queue, RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
};

pub struct Gpu {
    pub device: Device,
    pub queue: Queue,
    pub name: String,
}

impl Gpu {
    /// The high-performance adapter with timestamp queries when it has them; `None` with no
    /// adapter.
    pub fn new() -> Option<Self> {
        let instance = Instance::new(InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..InstanceDescriptor::new_without_display_handle()
        });
        let adapter =
            futures_lite::future::block_on(instance.request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                ..Default::default()
            }))
            .ok()?;
        let info = adapter.get_info();
        let (device, queue) =
            futures_lite::future::block_on(adapter.request_device(&DeviceDescriptor {
                label: Some("molang wgsl"),
                required_features: adapter.features() & Features::TIMESTAMP_QUERY,
                required_limits: adapter.limits(),
                ..Default::default()
            }))
            .ok()?;
        Some(Self {
            device,
            queue,
            name: format!("{} ({:?})", info.name, info.backend),
        })
    }

    pub fn pipeline(&self, source: &str) -> ComputePipeline {
        let module = self.device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(source.into()),
        });
        self.device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
    }

    pub fn buffer(&self, contents: &[u8], usage: BufferUsages) -> Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents,
                usage,
            })
    }

    pub fn bind(&self, pipeline: &ComputePipeline, buffers: &[&Buffer]) -> BindGroup {
        let entries: Vec<BindGroupEntry<'_>> = buffers
            .iter()
            .enumerate()
            .map(|(index, buffer)| BindGroupEntry {
                binding: index as u32,
                resource: buffer.as_entire_binding(),
            })
            .collect();
        self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        })
    }
}

/// Parses and validates a WGSL module with naga; the error is the rendered diagnostic.
pub fn validate(source: &str) -> Result<(), String> {
    let module =
        naga::front::wgsl::parse_str(source).map_err(|error| error.emit_to_string(source))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map(|_| ())
    .map_err(|error| error.emit_to_string(source))
}
