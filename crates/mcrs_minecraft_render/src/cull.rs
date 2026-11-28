use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::shader::ShaderDefVal;

use wgpu::ComputePassTimestampWrites;

use super::arenas::{Arenas, Geometry, HEADROOM};
use super::binds::Bindings;
use super::draws::{DrawList, PARAMS_STRIDE};
use super::hiz::Hiz;
use super::layer::LayerGroup;
use super::pipeline::MODEL_DEPTH_BIAS;
use super::shared_bindings::cull as cull_bindings;
use super::shared_types::DrawArgRow;
use super::stats::{DISPATCH_BYTES, DISPATCHES, args_reset, dispatch_offset};
use super::{Budget, DrawMask, PassSlot, PassTimestamps, VISIBLE_BYTES};

const VISIBLE_STEP: usize = 1 << 20;

pub(super) const CULL_THREADS: u32 = 32;
pub(super) const GROUP_THREADS: u32 = 256;
const GROUPS_PER_STEP: u32 = 2;

const _: () =
    assert!(mcrs_minecraft_mesh::GROUP_QUADS <= CULL_THREADS as usize && CULL_THREADS <= 32);

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

struct Lists {
    visible: Buffer,
    batches: Geometry,
    candidates: Geometry,
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

pub(super) struct Cull {
    lists: Lists,
    args_reset: Buffer,
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

fn cull_grid(limits: &WgpuLimits, threads: u32) -> u32 {
    const RESIDENT_INVOCATIONS: u32 = 1 << 19;
    let threads = threads
        .min(limits.max_compute_invocations_per_workgroup)
        .max(1);
    (RESIDENT_INVOCATIONS / threads)
        .min(limits.max_compute_workgroups_per_dimension)
        .max(1)
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
                &cull_bindings::ordered_entries(cull_bindings::OrderedEntries {
                    groups: &arenas.groups.words,
                    visible: &lists.visible,
                    args: &lists.args,
                    cave_visible: &lists.cave,
                    sections: &arenas.sections,
                    batches: &lists.batches.words,
                    hiz: &hiz.view,
                    candidates: &lists.candidates.words,
                }),
            ),
            quad: device.create_bind_group(
                "terrain quad cull",
                &cache.get_bind_group_layout(&layouts.quad),
                &cull_bindings::quad_entries(cull_bindings::QuadEntries {
                    groups: &arenas.groups.words,
                    visible: &lists.visible,
                    args: &lists.args,
                    cave_visible: &lists.cave,
                    sections: &arenas.sections,
                    hiz: &hiz.view,
                    candidates: &lists.candidates.words,
                    quad_words: &arenas.quads.words,
                    model_words: &arenas.vertices.words,
                }),
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
            ordered: cull_bindings::ordered_layout(),
            quad: cull_bindings::quad_layout(),
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

    pub fn rebind(&mut self, arenas: &Arenas, device: &RenderDevice, cache: &PipelineCache) {
        self.binds = Binds::new(&self.layouts, &self.lists, &self.hiz, arenas, device, cache);
    }

    pub fn visible(&self) -> &Buffer {
        &self.lists.visible
    }

    pub fn args(&self) -> &Buffer {
        &self.lists.args
    }

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

    pub fn second_pass(&self, list: &DrawList, pipeline_cache: &PipelineCache) -> bool {
        list.visible_entries != 0
            && pipeline_cache
                .get_compute_pipeline(self.pipelines.groups_second)
                .is_some()
    }

    #[cfg_attr(
        mcrs_lint,
        expect(mcrs::too_many_arguments, reason = "independent inputs of one cull")
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
                DrawArgRow::FirstPassQuadDispatch,
            );
        }
    }

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
                self.cull_indirect(
                    &mut pass,
                    binds,
                    list,
                    group,
                    groups,
                    mask,
                    DrawArgRow::CandidateDispatch,
                );
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
            self.cull_indirect(
                &mut pass,
                binds,
                list,
                group,
                quads,
                mask,
                DrawArgRow::SecondPassQuadDispatch,
            );
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

    #[cfg_attr(
        mcrs_lint,
        expect(
            mcrs::too_many_arguments,
            reason = "independent inputs of one layer group"
        )
    )]
    fn cull_indirect<'pass>(
        &'pass self,
        pass: &mut ComputePass<'pass>,
        binds: &'pass Bindings,
        list: &DrawList,
        group: LayerGroup,
        pipeline: &'pass ComputePipeline,
        mask: &DrawMask,
        row: DrawArgRow,
    ) {
        pass.set_pipeline(pipeline);
        for (index, draw) in list.drawn(group, mask) {
            let Some(offset) = dispatch_offset(row, draw.kind) else {
                continue;
            };
            pass.push_debug_group(draw.kind.label());
            pass.set_bind_group(0, &binds.view, &[index as u32 * PARAMS_STRIDE]);
            pass.dispatch_workgroups_indirect(&self.dispatch, offset);
            pass.pop_debug_group();
        }
    }
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

#[derive(Clone, Copy)]
struct Dispatch {
    per_workgroup: u32,
    grid: u32,
}

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
        if open != Some(draw.kind) {
            if open.is_some() {
                pass.pop_debug_group();
            }
            pass.push_debug_group(draw.kind.label());
            open = Some(draw.kind);
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
