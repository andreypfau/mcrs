use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::MainWorld;
use bevy::render::render_resource::{PipelineCache, TextureViewId};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture};
use mcrs_minecraft_render::{
    CameraOrigin, DEPTH_COMPARE, DeferredFrame, GBUFFER_FORMATS, Occlusion, PassSlot,
    PassTimestamps, Terrain, TerrainShade, clip_from_relative,
};

use crate::entity_assets::EntityAssets;
use crate::entity_gpu::{Capacities, DrawTargets, EntityGpu, PassQueries};
use crate::entity_library::EntityLibrary;
use crate::frame_uniform::{FrameUniform, FrameView};
use crate::height_threshold::HeightThreshold;
use crate::main_world::EntityClock;
use crate::slot_upload::{SCATTER_FRESH, SlotArena, SlotUpload};

/// The full rigs that one frame draws, in all and of one definition: the full-rig cap of the
/// native tier.
pub const FULL_RIG_CAP: u32 = 75_000;

/// The render world state of the entity passes.
#[derive(Resource, Default)]
pub(crate) struct EntityRenderState {
    generation: Option<u32>,
    library: Option<EntityLibrary>,
    gpu: Option<EntityGpu>,
    hiz: Option<TextureViewId>,
    pending: SlotUpload,
    clock: EntityClock,
    threshold: HeightThreshold,
}

pub(crate) fn extract_entities(mut main: ResMut<MainWorld>, mut state: ResMut<EntityRenderState>) {
    if let Some(clock) = main.get_resource::<EntityClock>() {
        state.clock = *clock;
    }
    if let Some(assets) = main.get_resource::<EntityAssets>()
        && state.generation != Some(assets.generation)
    {
        state.generation = Some(assets.generation);
        state.library = Some(assets.library.clone());
    }
    let Some(mut arena) = main.get_resource_mut::<SlotArena>() else {
        return;
    };
    let upload = arena.take_upload();
    merge(&mut state.pending, upload);
}

/// Adds the writes of `next` after those of `pending`. A slot keeps its last tick record and
/// stays fresh when an earlier write made it fresh.
fn merge(pending: &mut SlotUpload, next: SlotUpload) {
    if pending.ticks.is_empty()
        && pending.culls.is_empty()
        && pending.states.is_empty()
        && pending.inputs.is_empty()
    {
        *pending = next;
        return;
    }
    pending.slot_capacity = next.slot_capacity;
    pending.slot_count = next.slot_count;
    let mut at: HashMap<u32, usize> = pending
        .ticks
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.slot, index))
        .collect();
    for entry in next.ticks {
        match at
            .get(&entry.slot)
            .and_then(|index| pending.ticks.get_mut(*index))
        {
            Some(held) => {
                let fresh = held.tick & SCATTER_FRESH;
                *held = entry;
                held.tick |= fresh;
            }
            None => {
                at.insert(entry.slot, pending.ticks.len());
                pending.ticks.push(entry);
            }
        }
    }
    pending.culls.extend(next.culls);
    pending.states.extend(next.states);
    pending.inputs.extend(next.inputs);
}

pub(crate) fn prepare_entities(
    mut state: ResMut<EntityRenderState>,
    terrain: Option<Res<Terrain>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let pyramid = terrain.depth_pyramid();
    if let Some(library) = state.library.take() {
        let capacities = Capacities::for_cap(&library, FULL_RIG_CAP);
        let slots = state.pending.slot_capacity.max(1024);
        state.gpu = Some(EntityGpu::new(
            device.wgpu_device(),
            &library,
            capacities,
            slots,
            (**pyramid).clone(),
            DrawTargets {
                colors: GBUFFER_FORMATS,
                depth: bevy::render::render_resource::TextureFormat::Depth32Float,
                depth_compare: DEPTH_COMPARE,
            },
            &queue,
        ));
        state.hiz = Some(pyramid.id());
        info!(
            definitions = library.definitions.len(),
            models = library.models.len(),
            textures = library.variants.len(),
            "entity passes ready"
        );
        for diagnostic in &library.diagnostics {
            warn!("{diagnostic}");
        }
    }
    if state.hiz != Some(pyramid.id())
        && let Some(gpu) = state.gpu.as_mut()
    {
        gpu.set_hiz(device.wgpu_device(), (**pyramid).clone());
        state.hiz = Some(pyramid.id());
    }
}

/// The parts of the render world that the entity pass reads.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct EntityFrameParams<'w> {
    frame: Option<Res<'w, DeferredFrame>>,
    terrain: Option<Res<'w, Terrain>>,
    origin: Option<Res<'w, CameraOrigin>>,
    shade: Option<Res<'w, TerrainShade>>,
    occlusion: Res<'w, Occlusion>,
    cache: Res<'w, PipelineCache>,
    timestamps: Res<'w, PassTimestamps>,
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
}

/// The entity passes of a frame: the upload of the slots, the cull, the pose and the expand,
/// then the draw of the face records into the gbuffer.
pub(crate) fn draw_entities(
    view: ViewQuery<(&ExtractedView, &ViewDepthTexture)>,
    mut state: ResMut<EntityRenderState>,
    params: EntityFrameParams,
    mut ctx: RenderContext,
) {
    let (Some(frame), Some(terrain), Some(origin)) = (
        params.frame.as_deref(),
        params.terrain.as_deref(),
        params.origin.as_deref(),
    ) else {
        return;
    };
    let (extracted, depth) = view.into_inner();
    let pending = std::mem::take(&mut state.pending);
    let clock = state.clock;
    let mut threshold = state.threshold;
    let Some(gpu) = state.gpu.as_mut() else {
        state.pending = pending;
        return;
    };
    if let Some(counts) = gpu.take_counts() {
        threshold.update(counts.full_rigs, FULL_RIG_CAP);
    }
    let encoder = ctx.command_encoder();
    gpu.upload(
        params.device.wgpu_device(),
        &params.queue,
        encoder,
        &pending,
    );
    let clip_from_view = extracted.clip_from_view;
    let view = FrameView {
        clip_from_relative: clip_from_relative(
            clip_from_view,
            extracted.world_from_view.rotation(),
            origin.offset,
        ),
        camera_section: origin.section,
        camera_offset: origin.offset,
        viewport: extracted.viewport.zw().as_vec2().to_array(),
        focal_y: clip_from_view.y_axis.y,
    };
    let nether = params
        .shade
        .is_some_and(|shade| shade.0.shader_index() == 1);
    let mut uniform = FrameUniform::new(&view, clock.tick, clock.partial_tick, nether);
    uniform.hiz_levels = terrain.depth_pyramid_levels(&params.occlusion, &params.cache);
    uniform.height_threshold = threshold.pixels();
    gpu.write_frame(&params.queue, &uniform, pending.slot_count);
    let queries = params
        .timestamps
        .queries
        .as_ref()
        .map(|(set, first)| PassQueries {
            set,
            first: first + 2 * PassSlot::EntityScatter as u32,
        });
    gpu.encode_compute(encoder, queries);
    let color_attachments = frame.targets.each_ref().map(|target| {
        Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("entity gbuffer"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth.view(),
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    gpu.encode_draw(&mut pass);
    drop(pass);
    state.threshold = threshold;
}
