//! A headless adapter and the entity pack of the repository for the GPU tests.

use std::path::Path;

use mcrs_minecraft_entity_render::{
    Capacities, DrawTargets, EntityGpu, EntityLibrary, EntityPack, TextureImage,
};

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub name: String,
}

impl Gpu {
    /// The high-performance adapter with all its limits, and timestamps when it has them;
    /// `None` with no adapter.
    pub fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = futures_lite::future::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            },
        ))
        .ok()?;
        let info = adapter.get_info();
        let (device, queue) =
            futures_lite::future::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("entity render test"),
                required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
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

    /// A one-texel depth pyramid for a frame that tests none.
    pub fn no_pyramid(&self) -> wgpu::TextureView {
        self.device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("no pyramid"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }

    pub fn entity_gpu(
        &self,
        library: &EntityLibrary,
        capacities: Capacities,
        slots: u32,
    ) -> EntityGpu {
        EntityGpu::new(
            &self.device,
            library,
            capacities,
            slots,
            self.no_pyramid(),
            DrawTargets {
                colors: [
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::TextureFormat::Rgba16Float,
                    wgpu::TextureFormat::Rgba16Float,
                ],
                depth: wgpu::TextureFormat::Depth32Float,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
            },
            &self.queue,
        )
    }

    /// The bytes of `buffer`, after the queue has run.
    pub fn read(&self, buffer: &wgpu::Buffer) -> Vec<u8> {
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: buffer.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer.size());
        self.queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| result.unwrap());
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let bytes = slice.get_mapped_range().to_vec();
        staging.unmap();
        bytes
    }
}

/// The files of the entity pack of the repository.
pub fn pack_files() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/entity");
    let mut files = Vec::new();
    for folder in ["entity", "animations", "geometry", "render_controllers"] {
        for entry in std::fs::read_dir(root.join(folder)).unwrap().flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let name = format!("{folder}/{}", path.file_name().unwrap().to_string_lossy());
                files.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    files
}

pub fn pack() -> EntityPack {
    let files = pack_files();
    let pack = EntityPack::load(
        files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    );
    assert!(pack.diagnostics.is_empty(), "{:#?}", pack.diagnostics);
    pack
}

/// A white texture of 16 by 16 texels.
pub fn white_texture() -> TextureImage {
    TextureImage {
        identifier: "test:white".into(),
        width: 16,
        height: 16,
        rgba: vec![255; 16 * 16 * 4],
    }
}
