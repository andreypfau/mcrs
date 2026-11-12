use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, storage_buffer_sized, texture_2d,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::shader::ShaderDefVal;

use mcrs_minecraft_mesh::{STREAM_NAMES, STREAMS};
use wgpu::ComputePassTimestampWrites;

use super::arenas::{Arenas, Geometry, HEADROOM};
use super::binds::Bindings;
use super::draws::{DrawList, PARAMS_STRIDE};
use super::hiz::Hiz;
use super::layer::LayerGroup;
use super::pipeline::MODEL_DEPTH_BIAS;
use super::stats::{DISPATCH_BYTES, DISPATCHES, DRAW_ARGS_SIZE, args_reset};
use super::{Budget, DrawMask, PassSlot, PassTimestamps, VISIBLE_BYTES};

/// Entries the visible list starts at, and the granularity it grows in.
const VISIBLE_STEP: usize = 1 << 20;

pub(super) const CULL_THREADS: u32 = 32;
pub(super) const GROUP_THREADS: u32 = 256;
/// Groups a quad-cull workgroup tests at once.
const GROUPS_PER_STEP: u32 = 2;

// The quad cull gives each quad of a group a lane, and each quad a pass leaves a bit of one word.
const _: () =
    assert!(mcrs_minecraft_mesh::GROUP_QUADS <= CULL_THREADS as usize && CULL_THREADS <= 32);

/// Where each kernel reading a list finds its dispatch size in the copied table.
const QUADS_FIRST: u64 = 0;
const QUADS_SECOND: u64 = STREAMS as u64;
const GROUPS_SECOND: u64 = 2 * STREAMS as u64;

struct Pipelines {
    sections: CachedComputePipelineId,
    groups: CachedComputePipelineId,
    quads: CachedComputePipelineId,
    count: CachedComputePipelineId,
    scan: CachedComputePipelineId,
    scatter: CachedComputePipelineId,
    groups_second: CachedComputePipelineId,
    quads_second: CachedComputePipelineId,
}

/// The lists the kernels write and the draw arguments they count into.
struct Lists {
    /// A quad and its section per entry, which the vertex shader finds its quad through.
    visible: Buffer,
    /// One count per batch of `CULL_THREADS` groups, which the ordered cull turns into where
    /// each batch's survivors start.
    batches: Geometry,
    /// Three words per group: the quads the first cull pass left to the second, a place in the
    /// list of groups a pass's group cull kept, and one in the list of groups the first pass left
    /// quads of.
    candidates: Geometry,
    /// The cave walk's bits, then the frame's, which the cull writes.
    cave: Buffer,
    args: Buffer,
}

struct Layouts {
    ordered: BindGroupLayoutDescriptor,
    quad: BindGroupLayoutDescriptor,
}

struct Binds {
    ordered: BindGroup,
    quad: BindGroup,
}

/// What only the full path has: the compute passes that choose on the GPU what is drawn, the
/// lists and draw arguments they write, and the depth pyramid they test against.
pub(super) struct Cull {
    lists: Lists,
    // Copied over the args once a frame so the cull pass starts from zeroed instance counts
    // without one small clear per draw.
    args_reset: Buffer,
    /// The dispatch sizes the cull computes into the args, copied here because a dispatch cannot
    /// read its size from a buffer it may also write.
    dispatch: Buffer,
    pub hiz: Hiz,
    layouts: Layouts,
    binds: Binds,
    pipelines: Pipelines,
    cull_grid: u32,
    group_grid: u32,
}

fn batch_bytes(groups: usize) -> u64 {
    (groups.div_ceil(CULL_THREADS as usize) * 4) as u64
}

fn candidate_bytes(groups: usize) -> u64 {
    (groups * 12) as u64
}

fn visible_list(entries: usize, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("terrain visible list"),
        size: (entries * VISIBLE_BYTES) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// `cull.wesl` strides over the group arena, so the dispatch is capped by the device instead of
/// sized by the world and a bigger render distance can no longer walk it into wgpu's 65535
/// workgroups per dimension. wgpu reports no core count, so the cap is stated in invocations and
/// clamped by the limits it does report; a workgroup left with no group exits at once, but not
/// for free, so a draw holding fewer groups than the cap still dispatches only what it needs.
fn cull_grid(limits: &WgpuLimits, threads: u32) -> u32 {
    const RESIDENT_INVOCATIONS: u32 = 1 << 19;
    let threads = threads
        .min(limits.max_compute_invocations_per_workgroup)
        .max(1);
    (RESIDENT_INVOCATIONS / threads)
        .min(limits.max_compute_workgroups_per_dimension)
        .max(1)
}

fn ordered_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "terrain cull data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                texture_2d(TextureSampleType::Float { filterable: false }),
                storage_buffer_sized(false, None),
            ),
        ),
    )
}

/// The cull that tests quads one by one reads the geometry, and keeps under eight storage buffers
/// a stage by leaving out the batch counts only the ordered cull needs. The binding numbers are
/// the ones `cull.wesl` gives them.
fn quad_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "terrain quad cull data",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::COMPUTE,
            (
                (0, storage_buffer_read_only_sized(false, None)),
                (1, storage_buffer_sized(false, None)),
                (2, storage_buffer_sized(false, None)),
                (3, storage_buffer_sized(false, None)),
                (4, storage_buffer_read_only_sized(false, None)),
                (
                    6,
                    texture_2d(TextureSampleType::Float { filterable: false }),
                ),
                (7, storage_buffer_sized(false, None)),
                (8, storage_buffer_read_only_sized(false, None)),
                (9, storage_buffer_read_only_sized(false, None)),
            ),
        ),
    )
}

impl Pipelines {
    fn new(
        view_layout: &BindGroupLayoutDescriptor,
        layouts: &Layouts,
        section_slots: usize,
        device: &RenderDevice,
        asset_server: &AssetServer,
        pipeline_cache: &PipelineCache,
    ) -> Self {
        let shader: Handle<Shader> =
            asset_server.load("embedded://mcrs_minecraft_render/shaders/core/cull.wesl");
        let ordered = vec![view_layout.clone(), layouts.ordered.clone()];
        let quad = vec![view_layout.clone(), layouts.quad.clone()];
        let mut shader_defs = vec![
            ShaderDefVal::UInt("CULL_THREADS".into(), CULL_THREADS),
            ShaderDefVal::UInt("GROUP_THREADS".into(), GROUP_THREADS),
            ShaderDefVal::UInt("GROUPS_PER_STEP".into(), GROUPS_PER_STEP),
            ShaderDefVal::UInt("STREAMS".into(), STREAMS as u32),
            ShaderDefVal::UInt("SECTION_SLOTS".into(), section_slots as u32),
            ShaderDefVal::UInt("CAVE_WORDS".into(), section_slots.div_ceil(32) as u32),
            ShaderDefVal::UInt(
                "MAX_WORKGROUPS".into(),
                device.limits().max_compute_workgroups_per_dimension,
            ),
            ShaderDefVal::Int("MODEL_BIAS_CONSTANT".into(), MODEL_DEPTH_BIAS.constant),
            ShaderDefVal::UInt(
                "MODEL_BIAS_SLOPE_BITS".into(),
                MODEL_DEPTH_BIAS.slope_scale.to_bits(),
            ),
        ];
        if device.features().contains(WgpuFeatures::SUBGROUP) {
            shader_defs.push("SUBGROUPS".into());
        }
        let compute = |label: &str, layout: &Vec<BindGroupLayoutDescriptor>, entry: &str| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(label.to_owned().into()),
                layout: layout.clone(),
                shader: shader.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: Some(entry.to_owned().into()),
                // The cull reads its workgroup counters before it first writes them. Metal hands out
                // zeroed workgroup memory; DX12 and Vulkan hand out garbage unless wgpu zeroes it.
                zero_initialize_workgroup_memory: true,
                ..default()
            })
        };
        Self {
            sections: compute("terrain cull sections", &quad, "cull_sections"),
            groups: compute("terrain cull groups", &quad, "cull_groups"),
            quads: compute("terrain cull quads", &quad, "cull_quads"),
            count: compute("terrain cull count", &ordered, "count_ordered"),
            scan: compute("terrain cull scan", &ordered, "scan_ordered"),
            scatter: compute("terrain cull scatter", &ordered, "scatter_ordered"),
            groups_second: compute("terrain cull groups second", &quad, "cull_groups_second"),
            quads_second: compute("terrain cull quads second", &quad, "cull_quads_second"),
        }
    }
}

impl Binds {
    fn new(
        layouts: &Layouts,
        lists: &Lists,
        hiz: &Hiz,
        arenas: &Arenas,
        device: &RenderDevice,
        cache: &PipelineCache,
    ) -> Self {
        Self {
            ordered: device.create_bind_group(
                "terrain cull",
                &cache.get_bind_group_layout(&layouts.ordered),
                &BindGroupEntries::sequential((
                    arenas.groups.words.binding(),
                    lists.visible.as_entire_binding(),
                    lists.args.as_entire_binding(),
                    lists.cave.as_entire_binding(),
                    arenas.sections.binding(),
                    lists.batches.words.binding(),
                    &hiz.view,
                    lists.candidates.words.binding(),
                )),
            ),
            quad: device.create_bind_group(
                "terrain quad cull",
                &cache.get_bind_group_layout(&layouts.quad),
                &BindGroupEntries::with_indices((
                    (0, arenas.groups.words.binding()),
                    (1, lists.visible.as_entire_binding()),
                    (2, lists.args.as_entire_binding()),
                    (3, lists.cave.as_entire_binding()),
                    (4, arenas.sections.binding()),
                    (6, BindingResource::TextureView(&hiz.view)),
                    (7, lists.candidates.words.binding()),
                    (8, arenas.quads.words.binding()),
                    (9, arenas.vertices.words.binding()),
                )),
            ),
        }
    }
}

impl Cull {
    pub fn new(
        budget: &Budget,
        arenas: &Arenas,
        view_layout: &BindGroupLayoutDescriptor,
        device: &RenderDevice,
        asset_server: &AssetServer,
        pipeline_cache: &PipelineCache,
    ) -> Self {
        let args_init = args_reset();
        let storage = |label, limit| Geometry::records(label, limit, BufferUsages::STORAGE, device);
        let lists = Lists {
            visible: visible_list(VISIBLE_STEP, device),
            batches: storage("terrain cull batches", batch_bytes(budget.groups)),
            candidates: storage("terrain cull candidates", candidate_bytes(budget.groups)),
            cave: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain cave visibility"),
                contents: bytemuck::cast_slice(&vec![u32::MAX; 2 * budget.sections.div_ceil(32)]),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            }),
            args: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain draw args"),
                contents: bytemuck::cast_slice(&args_init),
                usage: BufferUsages::STORAGE
                    | BufferUsages::INDIRECT
                    | BufferUsages::COPY_DST
                    | BufferUsages::COPY_SRC,
            }),
        };
        let hiz = Hiz::new(device, asset_server, pipeline_cache);
        let layouts = Layouts {
            ordered: ordered_layout(),
            quad: quad_layout(),
        };
        let limits = device.limits();
        Self {
            binds: Binds::new(&layouts, &lists, &hiz, arenas, device, pipeline_cache),
            pipelines: Pipelines::new(
                view_layout,
                &layouts,
                budget.sections,
                device,
                asset_server,
                pipeline_cache,
            ),
            lists,
            args_reset: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain draw args reset"),
                contents: bytemuck::cast_slice(&args_init),
                usage: BufferUsages::COPY_SRC,
            }),
            dispatch: device.create_buffer(&BufferDescriptor {
                label: Some("terrain cull dispatch"),
                size: DISPATCH_BYTES,
                usage: BufferUsages::INDIRECT | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            hiz,
            layouts,
            cull_grid: cull_grid(&limits, CULL_THREADS),
            group_grid: cull_grid(&limits, GROUP_THREADS),
        }
    }

    /// Names the arrays as they stand in the two bind groups, after any of them was replaced.
    pub fn rebind(&mut self, arenas: &Arenas, device: &RenderDevice, cache: &PipelineCache) {
        self.binds = Binds::new(&self.layouts, &self.lists, &self.hiz, arenas, device, cache);
    }

    /// The quad and the section of each quad the cull kept, which a vertex finds its quad in.
    pub fn visible(&self) -> &Buffer {
        &self.lists.visible
    }

    /// The indirect arguments of each draw, as the cull counted them.
    pub fn args(&self) -> &Buffer {
        &self.lists.args
    }

    /// Grows the per-group lists to a group arena of `groups` records.
    pub fn fit_groups(
        &mut self,
        groups: usize,
        device: &RenderDevice,
        encoder: &mut CommandEncoder,
    ) {
        let lists = &mut self.lists;
        lists.batches.fit(batch_bytes(groups), device, encoder);
        lists
            .candidates
            .fit(candidate_bytes(groups), device, encoder);
    }

    pub fn holds_visible(&self, entries: usize) -> bool {
        (entries * VISIBLE_BYTES) as u64 <= self.lists.visible.size()
    }

    /// Replaces the visible list with one that holds `entries`. A new buffer is a new binding,
    /// so the caller rebuilds the bind groups that name it.
    ///
    /// The list is never shared out or clamped: a draw that does not fit would
    /// lose quads by their place in the arena, which reads as terrain blinking
    /// out rather than as a budget being spent.
    pub fn grow_visible(&mut self, entries: usize, device: &RenderDevice) {
        let entries = (entries + entries / HEADROOM)
            .next_multiple_of(VISIBLE_STEP)
            .max(VISIBLE_STEP);
        bevy::log::info!(entries, "growing the visible list");
        self.lists.visible = visible_list(entries, device);
    }

    pub fn cave(&self) -> &Buffer {
        &self.lists.cave
    }

    /// Whether a frame with `list` to draw has a second cull, and so a second set of draws.
    pub fn second_pass(&self, list: &DrawList, pipeline_cache: &PipelineCache) -> bool {
        list.visible_entries != 0
            && pipeline_cache
                .get_compute_pipeline(self.pipelines.groups_second)
                .is_some()
    }

    /// The first cull of a frame: against the frustum, the cave walk and the last frame's depth.
    #[expect(
        clippy::too_many_arguments,
        reason = "the view bindings, the draws and the section count of the terrain, the pipeline cache, the pass timestamps, the draw mask and the encoder of one cull"
    )]
    pub fn cull(
        &self,
        binds: &Bindings,
        list: &DrawList,
        section_slots: usize,
        pipeline_cache: &PipelineCache,
        timestamps: &PassTimestamps,
        mask: &DrawMask,
        encoder: &mut CommandEncoder,
    ) {
        let pipelines = &self.pipelines;
        let (Some(sections), Some(groups), Some(quads), Some(count), Some(scan), Some(scatter)) = (
            pipeline_cache.get_compute_pipeline(pipelines.sections),
            pipeline_cache.get_compute_pipeline(pipelines.groups),
            pipeline_cache.get_compute_pipeline(pipelines.quads),
            pipeline_cache.get_compute_pipeline(pipelines.count),
            pipeline_cache.get_compute_pipeline(pipelines.scan),
            pipeline_cache.get_compute_pipeline(pipelines.scatter),
        ) else {
            return;
        };

        let reset = self.args_reset.size();
        encoder.copy_buffer_to_buffer(&self.args_reset, 0, &self.lists.args, 0, reset);

        {
            let mut pass = timed_compute(
                encoder,
                "terrain cull sections",
                timestamps.compute(PassSlot::CullSections),
            );
            pass.set_pipeline(sections);
            pass.set_bind_group(0, &binds.view, &[0]);
            pass.set_bind_group(1, &self.binds.quad, &[]);
            pass.dispatch_workgroups(
                (section_slots as u32)
                    .div_ceil(CULL_THREADS)
                    .min(self.cull_grid),
                1,
                1,
            );
        }
        // Blending is not commutative, so translucent draws have to reach the rasteriser in the order
        // the list holds them; opaque ones may be compacted.
        {
            let mut pass = timed_compute(
                encoder,
                "terrain cull blended",
                timestamps.compute(PassSlot::CullBlended),
            );
            pass.set_bind_group(1, &self.binds.ordered, &[]);
            let blended = LayerGroup::Translucent;
            let batched = Dispatch {
                per_workgroup: CULL_THREADS,
                grid: self.cull_grid,
            };
            cull_group(&mut pass, binds, list, blended, count, mask, batched);
            pass.set_pipeline(scan);
            for (index, _) in list.drawn(blended, mask) {
                pass.set_bind_group(0, &binds.view, &[index as u32 * PARAMS_STRIDE]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            cull_group(&mut pass, binds, list, blended, scatter, mask, batched);
        }
        {
            let mut pass =
                timed_compute(encoder, "terrain cull", timestamps.compute(PassSlot::Cull));
            pass.set_bind_group(1, &self.binds.quad, &[]);
            let grouped = Dispatch {
                per_workgroup: GROUP_THREADS,
                grid: self.group_grid,
            };
            cull_group(
                &mut pass,
                binds,
                list,
                LayerGroup::Opaque,
                groups,
                mask,
                grouped,
            );
        }
        self.copy_dispatches(encoder);
        {
            let mut pass = timed_compute(
                encoder,
                "terrain cull quads",
                timestamps.compute(PassSlot::CullQuads),
            );
            pass.set_bind_group(1, &self.binds.quad, &[]);
            self.cull_indirect(
                &mut pass,
                binds,
                list,
                LayerGroup::Opaque,
                quads,
                mask,
                QUADS_FIRST,
            );
        }
    }

    /// Rebuilds the pyramid from what the first pass drew and culls again against it, which
    /// brings back what only the last frame's depth hid.
    pub fn cull_second(
        &self,
        binds: &Bindings,
        list: &DrawList,
        pipeline_cache: &PipelineCache,
        timestamps: &PassTimestamps,
        mask: &DrawMask,
        encoder: &mut CommandEncoder,
    ) {
        self.hiz
            .build(pipeline_cache, timestamps.compute(PassSlot::Hiz), encoder);
        let (Some(groups), Some(quads)) = (
            pipeline_cache.get_compute_pipeline(self.pipelines.groups_second),
            pipeline_cache.get_compute_pipeline(self.pipelines.quads_second),
        ) else {
            return;
        };
        self.copy_dispatches(encoder);
        {
            let mut pass = timed_compute(
                encoder,
                "terrain cull second",
                timestamps.compute(PassSlot::CullSecond),
            );
            pass.set_bind_group(1, &self.binds.quad, &[]);
            for group in LayerGroup::ALL {
                self.cull_indirect(&mut pass, binds, list, group, groups, mask, GROUPS_SECOND);
            }
        }
        self.copy_dispatches(encoder);
        let mut pass = timed_compute(
            encoder,
            "terrain cull quads second",
            timestamps.compute(PassSlot::CullQuadsSecond),
        );
        pass.set_bind_group(1, &self.binds.quad, &[]);
        for group in LayerGroup::ALL {
            self.cull_indirect(&mut pass, binds, list, group, quads, mask, QUADS_SECOND);
        }
    }

    fn copy_dispatches(&self, encoder: &mut CommandEncoder) {
        encoder.copy_buffer_to_buffer(
            &self.lists.args,
            DISPATCHES,
            &self.dispatch,
            0,
            DISPATCH_BYTES,
        );
    }

    /// Dispatches `pipeline` once a draw of `group`, as many workgroups as the cull counted into
    /// the draw's row of `table`.
    #[expect(
        clippy::too_many_arguments,
        reason = "the pass, the view bindings and the draws of one layer group, the kernel, the draw mask and the row of the dispatch table"
    )]
    fn cull_indirect<'pass>(
        &'pass self,
        pass: &mut ComputePass<'pass>,
        binds: &'pass Bindings,
        list: &DrawList,
        group: LayerGroup,
        pipeline: &'pass ComputePipeline,
        mask: &DrawMask,
        table: u64,
    ) {
        pass.set_pipeline(pipeline);
        for (index, draw) in list.drawn(group, mask) {
            pass.push_debug_group(stream_name(draw.stream));
            pass.set_bind_group(0, &binds.view, &[index as u32 * PARAMS_STRIDE]);
            pass.dispatch_workgroups_indirect(
                &self.dispatch,
                (table + index as u64) * DRAW_ARGS_SIZE,
            );
            pass.pop_debug_group();
        }
    }
}

pub(super) fn stream_name(stream: u32) -> &'static str {
    STREAM_NAMES
        .get(stream as usize)
        .copied()
        .unwrap_or("terrain")
}

fn timed_compute<'e>(
    encoder: &'e mut CommandEncoder,
    label: &'static str,
    timestamp_writes: Option<ComputePassTimestampWrites<'_>>,
) -> ComputePass<'e> {
    encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some(label),
        timestamp_writes,
    })
}

/// A workgroup to every `per_workgroup` groups of a draw, up to `grid` workgroups.
#[derive(Clone, Copy)]
struct Dispatch {
    per_workgroup: u32,
    grid: u32,
}

/// Dispatches `pipeline` once a draw of `group`.
fn cull_group<'pass>(
    pass: &mut ComputePass<'pass>,
    binds: &'pass Bindings,
    list: &DrawList,
    group: LayerGroup,
    pipeline: &'pass ComputePipeline,
    mask: &DrawMask,
    dispatch: Dispatch,
) {
    pass.set_pipeline(pipeline);
    let mut open = None;
    for (index, draw) in list.drawn(group, mask) {
        if open != Some(draw.stream) {
            if open.is_some() {
                pass.pop_debug_group();
            }
            pass.push_debug_group(stream_name(draw.stream));
            open = Some(draw.stream);
        }
        pass.set_bind_group(0, &binds.view, &[index as u32 * PARAMS_STRIDE]);
        pass.dispatch_workgroups(
            draw.group_count
                .div_ceil(dispatch.per_workgroup)
                .min(dispatch.grid),
            1,
            1,
        );
    }
    if open.is_some() {
        pass.pop_debug_group();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cull_grid_fits_a_dispatch_whatever_the_device_reports() {
        for limits in [
            WgpuLimits::downlevel_webgl2_defaults(),
            WgpuLimits::downlevel_defaults(),
            WgpuLimits::default(),
            WgpuLimits {
                max_compute_workgroups_per_dimension: 1,
                ..WgpuLimits::default()
            },
        ] {
            for threads in [CULL_THREADS, GROUP_THREADS] {
                let grid = cull_grid(&limits, threads);
                assert!(grid >= 1);
                assert!(grid <= limits.max_compute_workgroups_per_dimension.max(1));
            }
        }
    }
}
