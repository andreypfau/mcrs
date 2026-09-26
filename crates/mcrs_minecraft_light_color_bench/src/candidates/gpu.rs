use std::time::Duration;

use futures_lite::future::block_on;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::region::{EdgeCosts, Region, section_output};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use super::cut;
use crate::fixture::Scene;

/// Emission is at most 15 and every step costs at least 1, so after 15 waves
/// no level can rise.
const WAVES: usize = 15;
const WORKGROUP: u32 = 64;
const TIMESTAMP_BYTES: u64 = 2 * 8;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    waves: wgpu::ComputePipeline,
    timestamps: wgpu::QuerySet,
    period_ns: f64,
}

impl Gpu {
    pub fn new() -> Result<Gpu, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| format!("no Metal adapter: {e}"))?;
        if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Err(format!(
                "{} lacks TIMESTAMP_QUERY, so passes cannot be timed",
                adapter.get_info().name
            ));
        }
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("light colour"),
            required_features: wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        }))
        .map_err(|e| format!("{}: {e}", adapter.get_info().name))?;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("light colour"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let waves = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("waves"),
            layout: None,
            module: &module,
            entry_point: Some("waves"),
            compilation_options: Default::default(),
            cache: None,
        });
        let timestamps = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("light colour pass"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let period_ns = queue.get_timestamp_period() as f64;

        // Metal reads back zeros from the first pass that samples a counter
        // buffer, so that pass must not be a measured one.
        let gpu = Gpu {
            device,
            queue,
            waves,
            timestamps,
            period_ns,
        };
        // Metal reads back zero timestamps around the first dispatch a device
        // runs, so that dispatch must not be a measured one.
        gpu.propagate(bytemuck::cast_slice(&[1u32, 1, 0, 0]), &[0], &[0]);
        Ok(gpu)
    }

    /// Light of type `t` on the section's 18³ output, and the GPU time of the
    /// waves that propagated it over the section's region.
    pub fn run_one_lane(
        &self,
        scene: &Scene,
        section: SectionPos,
        t: LightType,
    ) -> (Vec<u8>, Duration) {
        let (registry, colours) = (&scene.registry, &scene.colours);
        let (min, size) = section_output(section);
        let region = Region::new(min, size, scene.bounds, registry, scene.cells());
        let costs = EdgeCosts::new(&region, registry);
        let cost: Vec<u32> = costs
            .entry
            .iter()
            .zip(&costs.veto)
            .map(|(&entry, &veto)| entry as u32 | (veto as u32) << 4)
            .collect();
        let seeds: Vec<u32> = region
            .blocks
            .iter()
            .map(|&id| {
                if colours.light_type(id) == t {
                    registry.emission(id).get() as u32
                } else {
                    0
                }
            })
            .collect();

        let params = [region.size as u32, 1, 0, 0];
        let (levels, elapsed) = self.propagate(bytemuck::cast_slice(&params), &cost, &seeds);
        let elapsed = elapsed.expect("the pass has ordered timestamps");
        let levels = cut(&region, min, size, |cell| levels[cell] as u8);
        (levels, elapsed)
    }

    fn propagate(
        &self,
        params: &[u8],
        cost: &[u32],
        seeds: &[u32],
    ) -> (Vec<u32>, Option<Duration>) {
        let device = &self.device;
        let storage = |label, contents: &[u32], usage| {
            device.create_buffer_init(&BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(contents),
                usage: wgpu::BufferUsages::STORAGE | usage,
            })
        };
        let params = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("params"),
            contents: params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let cost = storage("cost", cost, wgpu::BufferUsages::empty());
        let ping = storage("ping", seeds, wgpu::BufferUsages::empty());
        let pong = storage("pong", seeds, wgpu::BufferUsages::COPY_SRC);
        let layout = self.waves.get_bind_group_layout(0);
        let bind = |from: &wgpu::Buffer, to: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waves"),
                layout: &layout,
                entries: &[
                    entry(0, &params),
                    entry(1, &cost),
                    entry(2, from),
                    entry(3, to),
                ],
            })
        };
        let forth = bind(&ping, &pong);
        let back = bind(&pong, &ping);

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("waves"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &self.timestamps,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            pass.set_pipeline(&self.waves);
            let groups = (seeds.len() as u32).div_ceil(WORKGROUP);
            for wave in 0..WAVES {
                pass.set_bind_group(0, if wave % 2 == 0 { &forth } else { &back }, &[]);
                pass.dispatch_workgroups(groups, 1, 1);
            }
        }
        let result = if WAVES % 2 == 1 { &pong } else { &ping };
        let times = self.read_timestamps(&mut encoder);
        let lanes = self.readback(&mut encoder, result, result.size());
        self.queue.submit([encoder.finish()]);

        let lanes = self.map(&lanes);
        let times: Vec<u64> = bytemuck::cast_slice(&self.map(&times)).to_vec();
        let elapsed = (times[1] > times[0])
            .then(|| Duration::from_nanos(((times[1] - times[0]) as f64 * self.period_ns) as u64));
        (bytemuck::cast_slice(&lanes).to_vec(), elapsed)
    }

    fn read_timestamps(&self, encoder: &mut wgpu::CommandEncoder) -> wgpu::Buffer {
        let resolved = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamps"),
            size: TIMESTAMP_BYTES,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        encoder.resolve_query_set(&self.timestamps, 0..2, &resolved, 0);
        self.readback(encoder, &resolved, TIMESTAMP_BYTES)
    }

    fn readback(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Buffer,
        size: u64,
    ) -> wgpu::Buffer {
        let target = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(source, 0, &target, 0, size);
        target
    }

    fn map(&self, buffer: &wgpu::Buffer) -> Vec<u8> {
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("a readback buffer maps");
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device finishes its work");
        let bytes = slice.get_mapped_range().to_vec();
        buffer.unmap();
        bytes
    }
}

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}
