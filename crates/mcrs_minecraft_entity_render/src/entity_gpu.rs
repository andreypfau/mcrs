use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, BufferDescriptor,
    BufferUsages, CommandEncoder, ComputePassDescriptor, ComputePassTimestampWrites, Device, Queue,
    RenderPass, TextureFormat,
};

use crate::counts_readback::{CountsReadback, EntityFrameCounts};
use crate::entity_library::{EntityLibrary, MAX_ARRAYS, MAX_DEFINITIONS, VariantRecord};
use crate::frame_uniform::FrameUniform;
use crate::gpu_buffers::{
    COUNTERS, FrameBuffers, SlotBuffers, StaticBuffers, impostor_draws, impostor_list,
};
use crate::gpu_layouts::{ARGS, CULL, DRAW, EXPAND, Layouts, POSE, PREPARE, SCATTER, Slot};
use crate::gpu_pipelines::{Pipelines, texture_arrays};
use crate::impostor_atlas::{BakeResources, ImpostorAtlas};
use crate::records::{CullRecord, InputRecord, ScatterEntry, StateRecord};
use crate::slot_upload::SlotUpload;

/// How many mobs, bones and faces the frame buffers of the entity passes hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacities {
    /// The full rigs of one definition in one frame.
    pub full_rigs: u32,
    /// The bone matrices of one frame.
    pub bones: u32,
    /// The face records of one texture array in one frame.
    pub faces: u32,
}

impl Capacities {
    /// The capacities of the full-rig cap `full_rigs` for the definitions of `library`.
    #[must_use]
    pub fn for_cap(library: &EntityLibrary, full_rigs: u32) -> Self {
        let bones = library
            .definitions
            .iter()
            .map(|definition| definition.max_bones)
            .max()
            .unwrap_or(1);
        let lanes = library
            .definitions
            .iter()
            .map(|definition| definition.expand_lanes)
            .max()
            .unwrap_or(1);
        Self {
            full_rigs,
            bones: full_rigs.saturating_mul(bones),
            faces: full_rigs.saturating_mul(lanes).saturating_mul(3),
        }
    }
}

/// Timestamps around each compute pass of the entity passes.
#[derive(Clone, Copy, Debug)]
pub struct PassQueries<'a> {
    pub set: &'a wgpu::QuerySet,
    /// The first query; the passes take two queries each, in the order of [`ComputePass`].
    pub first: u32,
}

/// A compute pass of the entity passes, in the order in which they run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputePass {
    Scatter,
    Cull,
    Pose,
    Expand,
}

impl ComputePass {
    pub const ALL: [Self; 4] = [Self::Scatter, Self::Cull, Self::Pose, Self::Expand];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Scatter => "entity scatter",
            Self::Cull => "entity cull",
            Self::Pose => "entity pose",
            Self::Expand => "entity expand",
        }
    }
}

/// The GPU side of the entity passes on the full execution path: compute shaders and storage
/// buffers in the vertex stage.
pub struct EntityGpu {
    capacities: Capacities,
    layouts: Layouts,
    pipelines: Pipelines,
    /// The bind group of each texture array.
    arrays: Vec<BindGroup>,
    array_textures: Vec<wgpu::Texture>,
    /// The array and the layer of each texture, for the writes of its texels.
    variants: Vec<VariantRecord>,
    statics: StaticBuffers,
    slots: SlotBuffers,
    frame: FrameBuffers,
    hiz: wgpu::TextureView,
    atlas: ImpostorAtlas,
    readback: CountsReadback,
    binds: Binds,
    cull_grid: u32,
    scatter_count: u32,
}

/// The color formats and the depth format of the targets of the entity draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawTargets {
    pub colors: [TextureFormat; 3],
    pub depth: TextureFormat,
    pub depth_compare: wgpu::CompareFunction,
}

impl EntityGpu {
    /// The GPU state of `library`, with `slot_capacity` slots to start and the frame capacities
    /// `capacities`. `hiz` is the depth pyramid that the cull reads; a frame with
    /// `hiz_levels` 0 tests none.
    #[must_use]
    pub fn new(
        device: &Device,
        library: &EntityLibrary,
        capacities: Capacities,
        slot_capacity: u32,
        hiz: wgpu::TextureView,
        targets: DrawTargets,
        queue: &Queue,
    ) -> Self {
        let layouts = Layouts::new(device);
        let pipelines = Pipelines::new(device, library, &layouts, targets);
        let (array_textures, arrays) = texture_arrays(device, queue, library, &layouts.texture)
            .into_iter()
            .unzip();
        let statics = StaticBuffers::new(device, library);
        let slots = SlotBuffers::new(device, slot_capacity);
        let frame = FrameBuffers::new(
            device,
            capacities,
            slot_capacity.max(1024),
            library.definitions.len(),
            library.arrays.len(),
        );
        let limits = device.limits();
        let cull_grid = ((1u32 << 19) / 64)
            .min(limits.max_compute_workgroups_per_dimension)
            .max(1);
        let atlas = ImpostorAtlas::new(device, library, &layouts.impostor_textures);
        let binds = Binds::new(device, &layouts, &slots, &statics, &frame, &atlas, &hiz);
        Self {
            capacities,
            binds,
            atlas,
            readback: CountsReadback::new(device),
            layouts,
            pipelines,
            arrays,
            array_textures,
            variants: library.variants.clone(),
            statics,
            slots,
            frame,
            hiz,
            cull_grid,
            scatter_count: 0,
        }
    }

    fn rebind(&mut self, device: &Device) {
        self.binds = Binds::new(
            device,
            &self.layouts,
            &self.slots,
            &self.statics,
            &self.frame,
            &self.atlas,
            &self.hiz,
        );
    }

    /// Replaces the depth pyramid that the cull reads.
    pub fn set_hiz(&mut self, device: &Device, hiz: wgpu::TextureView) {
        self.hiz = hiz;
        self.rebind(device);
    }

    #[must_use]
    pub fn capacities(&self) -> Capacities {
        self.capacities
    }

    /// The slots that the slot buffers hold.
    #[must_use]
    pub fn slot_capacity(&self) -> u32 {
        self.slots.capacity
    }

    /// The state records of the slots.
    #[must_use]
    pub fn slot_states(&self) -> &Buffer {
        &self.slots.states
    }

    /// The counters of the last frame: bones, then the visible count of each definition, then
    /// the face count of each array, then the mobs that did not fit.
    #[must_use]
    pub fn counters(&self) -> &Buffer {
        &self.frame.counters
    }

    /// The bone matrices, three rows of four `f32` for each bone.
    #[must_use]
    pub fn bone_matrices(&self) -> &Buffer {
        &self.frame.bone_matrices
    }

    /// The visible list: for each definition, `full_rigs` entries of a slot and its first bone.
    #[must_use]
    pub fn visible(&self) -> &Buffer {
        &self.frame.visible
    }

    /// Grows the slot buffers to `upload.slot_capacity` and writes the records of `upload`.
    /// The grow copy has its own submission: it must run before the queue writes of this call.
    pub fn upload(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        upload: &SlotUpload,
    ) {
        if upload.slot_capacity > self.slots.capacity {
            let grown = SlotBuffers::new(device, upload.slot_capacity.next_power_of_two());
            let mut grow = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("entity slot grow"),
            });
            self.slots.copy_into(&grown, &mut grow);
            queue.submit([grow.finish()]);
            self.frame.impostors = impostor_list(device, grown.capacity);
            self.frame.impostor_draws = impostor_draws(device, grown.capacity);
            self.slots = grown;
            self.rebind(device);
        }
        let ticks = upload.ticks.len() as u32;
        if ticks > self.frame.scatter_capacity {
            self.frame.scatter = device.create_buffer(&BufferDescriptor {
                label: Some("entity scatter"),
                size: u64::from(ticks.next_power_of_two()) * size_of::<ScatterEntry>() as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.frame.scatter_capacity = ticks.next_power_of_two();
            self.rebind(device);
        }
        if !upload.ticks.is_empty() {
            queue.write_buffer(&self.frame.scatter, 0, bytemuck::cast_slice(&upload.ticks));
        }
        self.scatter_count = ticks;
        for range in &upload.culls {
            queue.write_buffer(
                &self.slots.culls,
                u64::from(range.first) * size_of::<CullRecord>() as u64,
                bytemuck::cast_slice(&range.records),
            );
        }
        for range in &upload.states {
            queue.write_buffer(
                &self.slots.states,
                u64::from(range.first) * size_of::<StateRecord>() as u64,
                bytemuck::cast_slice(&range.records),
            );
        }
        for range in &upload.inputs {
            queue.write_buffer(
                &self.slots.inputs,
                u64::from(range.first) * size_of::<InputRecord>() as u64,
                bytemuck::cast_slice(&range.records),
            );
        }
        for record in upload.states.iter().flat_map(|range| &range.records) {
            self.atlas.note(record);
        }
        self.atlas.bake_noted(
            device,
            queue,
            encoder,
            &BakeResources {
                pipeline: &self.pipelines.bake,
                draw_layout: &self.layouts.draw,
                images_layout: &self.layouts.impostor_textures,
                uniform: &self.frame.uniform,
                models: &self.statics.models,
                cuboids: &self.statics.cuboids,
                variants: &self.statics.variants,
                arrays: &self.arrays,
            },
        );
    }

    /// Writes the RGBA texels of the texture `variant`, which fill its image from the top left.
    /// A variant that the library does not hold or texels of another size change nothing.
    pub fn write_texels(&self, queue: &Queue, variant: u16, rgba: &[u8]) {
        let Some(record) = self.variants.get(usize::from(variant)) else {
            return;
        };
        let Some(texture) = self.array_textures.get((record.array_layer >> 16) as usize) else {
            return;
        };
        let side = texture.width();
        let width = (record.uv_scale[0] * side as f32).round() as u32;
        let height = (record.uv_scale[1] * side as f32).round() as u32;
        if rgba.len() != (width * height * 4) as usize {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: record.array_layer & 0xffff,
                },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// The image sets of the impostors that the bake has rendered.
    #[must_use]
    pub fn impostor_sets(&self) -> u32 {
        self.atlas.image_sets()
    }

    /// The counts of the latest earlier frame whose readback completed. Call it once a frame,
    /// before [`Self::encode_compute`].
    pub fn take_counts(&mut self) -> Option<EntityFrameCounts> {
        self.readback.take()
    }

    /// Writes the frame uniform, with the capacities and the counts of the last upload.
    pub fn write_frame(&self, queue: &Queue, frame: &FrameUniform, slot_count: u32) {
        let frame = FrameUniform {
            slot_count: slot_count.min(self.slots.capacity),
            scatter_count: self.scatter_count,
            bone_capacity: self.capacities.bones,
            visible_capacity: self.capacities.full_rigs,
            face_capacity: self.capacities.faces,
            ..*frame
        };
        queue.write_buffer(&self.frame.uniform, 0, bytemuck::bytes_of(&frame));
    }

    /// Encodes the compute passes of a frame: the scatter of the tick records, the cull, the
    /// pose and the expand of each definition.
    pub fn encode_compute(
        &mut self,
        encoder: &mut CommandEncoder,
        queries: Option<PassQueries<'_>>,
    ) {
        encoder.copy_buffer_to_buffer(
            &self.frame.counters_zero,
            0,
            &self.frame.counters,
            0,
            COUNTERS * 4,
        );
        let writes = |pass: ComputePass| {
            queries.map(|queries| {
                let begin = queries.first + 2 * pass as u32;
                ComputePassTimestampWrites {
                    query_set: queries.set,
                    beginning_of_pass_write_index: Some(begin),
                    end_of_pass_write_index: Some(begin + 1),
                }
            })
        };
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some(ComputePass::Scatter.name()),
                timestamp_writes: writes(ComputePass::Scatter),
            });
            if self.scatter_count > 0 {
                pass.set_pipeline(&self.pipelines.scatter);
                pass.set_bind_group(0, &self.binds.scatter, &[]);
                pass.dispatch_workgroups(self.scatter_count.div_ceil(64), 1, 1);
            }
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some(ComputePass::Cull.name()),
                timestamp_writes: writes(ComputePass::Cull),
            });
            pass.set_pipeline(&self.pipelines.cull);
            pass.set_bind_group(0, &self.binds.cull, &[]);
            pass.dispatch_workgroups(self.slots.capacity.div_ceil(64).min(self.cull_grid), 1, 1);
            pass.set_pipeline(&self.pipelines.dispatch_args);
            pass.set_bind_group(0, &self.binds.args, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some(ComputePass::Pose.name()),
                timestamp_writes: writes(ComputePass::Pose),
            });
            pass.set_bind_group(0, &self.binds.pose, &[]);
            for (index, definition) in self.pipelines.definitions.iter().enumerate() {
                if let Some(pose) = &definition.pose {
                    pass.set_pipeline(pose);
                    pass.dispatch_workgroups_indirect(&self.frame.indirect, index as u64 * 12);
                }
            }
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some(ComputePass::Expand.name()),
                timestamp_writes: writes(ComputePass::Expand),
            });
            pass.set_bind_group(0, &self.binds.expand, &[]);
            let expand_base =
                MAX_DEFINITIONS.min(self.pipelines.definitions.len().max(1)) as u64 * 12;
            for (index, definition) in self.pipelines.definitions.iter().enumerate() {
                if definition.pose.is_some() {
                    pass.set_pipeline(&definition.expand);
                    pass.dispatch_workgroups_indirect(
                        &self.frame.indirect,
                        expand_base + index as u64 * 12,
                    );
                }
            }
            pass.set_pipeline(&self.pipelines.draw_args);
            pass.set_bind_group(0, &self.binds.args, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("entity impostor prepare"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipelines.impostor_prepare);
            pass.set_bind_group(0, &self.binds.prepare, &[]);
            pass.dispatch_workgroups_indirect(&self.frame.indirect, self.impostor_args() + 16);
            pass.set_pipeline(&self.pipelines.impostor_draw_args);
            pass.set_bind_group(0, &self.binds.args, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.readback.encode(&self.frame.counters, encoder);
    }

    /// Draws the face records of each texture array, then the impostors, into the render pass
    /// of the gbuffer.
    pub fn encode_draw(&self, pass: &mut RenderPass<'_>) {
        self.encode_draw_full_rigs(pass);
        self.encode_draw_impostors(pass);
    }

    /// Draws the face records of each texture array into the render pass of the gbuffer: first
    /// the faces with no cutout texel, then the others.
    pub fn encode_draw_full_rigs(&self, pass: &mut RenderPass<'_>) {
        pass.set_pipeline(&self.pipelines.draw_opaque);
        pass.set_bind_group(0, &self.binds.draw, &[]);
        for (index, array) in self.arrays.iter().enumerate() {
            pass.set_bind_group(1, array, &[]);
            pass.draw_indirect(&self.frame.indirect, self.draw_base() + index as u64 * 16);
        }
        pass.set_pipeline(&self.pipelines.draw);
        let cutout_base = self.impostor_args() + 48;
        for (index, array) in self.arrays.iter().enumerate() {
            pass.set_bind_group(1, array, &[]);
            pass.draw_indirect(&self.frame.indirect, cutout_base + index as u64 * 16);
        }
    }

    /// Draws the impostors into the render pass of the gbuffer.
    pub fn encode_draw_impostors(&self, pass: &mut RenderPass<'_>) {
        pass.set_pipeline(&self.pipelines.impostor);
        pass.set_bind_group(0, &self.binds.impostor, &[]);
        pass.set_bind_group(1, &self.atlas.images, &[]);
        pass.draw_indirect(&self.frame.indirect, self.impostor_args());
        pass.set_pipeline(&self.pipelines.impostor_small);
        pass.draw_indirect(&self.frame.indirect, self.impostor_args() + 32);
    }

    /// The byte offset of the draw arguments of the impostors; the dispatch arguments of their
    /// prepare pass follow them.
    fn impostor_args(&self) -> u64 {
        self.draw_base() + self.arrays.len().clamp(1, MAX_ARRAYS) as u64 * 16
    }

    /// The byte offset of the draw arguments of the first texture array.
    fn draw_base(&self) -> u64 {
        self.pipelines.definitions.len().max(1) as u64 * 24
    }
}

/// The bind group of each pass.
struct Binds {
    scatter: BindGroup,
    cull: BindGroup,
    pose: BindGroup,
    expand: BindGroup,
    prepare: BindGroup,
    args: BindGroup,
    draw: BindGroup,
    impostor: BindGroup,
}

impl Binds {
    fn new(
        device: &Device,
        layouts: &Layouts,
        s: &SlotBuffers,
        st: &StaticBuffers,
        f: &FrameBuffers,
        atlas: &ImpostorAtlas,
        hiz: &wgpu::TextureView,
    ) -> Self {
        let buffers: [(u32, &Buffer); 22] = [
            (0, &f.uniform),
            (1, &s.ticks),
            (3, &s.tick_of_record),
            (4, &s.culls),
            (5, &s.states),
            (6, &s.inputs),
            (8, &st.models),
            (9, &st.bones),
            (10, &st.cuboids),
            (11, &f.visible),
            (12, &f.bone_matrices),
            (13, &f.instances),
            (14, &f.faces),
            (15, &f.counters),
            (16, &f.indirect),
            (17, &f.scatter),
            (19, &st.variants),
            (20, &st.cutout),
            (21, &f.impostors),
            (22, &atlas.pairs),
            (24, &f.impostor_draws),
            (25, &atlas.covers),
        ];
        let bind = |label: &str, layout: &BindGroupLayout, bindings: &[(u32, Slot)]| {
            let entries: Vec<BindGroupEntry<'_>> = bindings
                .iter()
                .filter_map(|&(binding, slot)| {
                    let resource = match slot {
                        Slot::Pyramid => BindingResource::TextureView(hiz),
                        _ => buffers
                            .iter()
                            .find(|(at, _)| *at == binding)?
                            .1
                            .as_entire_binding(),
                    };
                    Some(BindGroupEntry { binding, resource })
                })
                .collect();
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &entries,
            })
        };
        Self {
            scatter: bind("entity scatter", &layouts.scatter, &SCATTER),
            cull: bind("entity cull", &layouts.cull, &CULL),
            pose: bind("entity pose", &layouts.pose, &POSE),
            expand: bind("entity expand", &layouts.expand, &EXPAND),
            prepare: bind("entity impostor prepare", &layouts.prepare, &PREPARE),
            args: bind("entity args", &layouts.args, &ARGS),
            draw: bind("entity draw", &layouts.draw, &DRAW),
            impostor: bind(
                "entity impostor",
                &layouts.impostor,
                &[(0, Slot::Frame), (24, Slot::Storage { read_only: true })],
            ),
        }
    }
}
