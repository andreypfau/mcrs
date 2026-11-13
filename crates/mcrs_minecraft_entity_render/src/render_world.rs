use std::collections::HashMap;

use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::MainWorld;
use bevy::render::render_resource::{PipelineCache, TextureViewId};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::ViewDepthStencilTexture;
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_render::{
    DEPTH_COMPARE, DeferredFrame, FrameView, GBUFFER_FORMATS, Occlusion, PassSlot, PassTimestamps,
    Terrain, TerrainShade,
};

use crate::entity_assets::EntityAssets;
use crate::entity_gpu::{Capacities, DrawTargets, EntityGpu, PassQueries};
use crate::entity_library::EntityLibrary;
use crate::frame_uniform::FrameUniform;
use crate::height_threshold::HeightThreshold;
use crate::main_world::EntityClock;
use crate::player_skin::{SkinSlots, SkinUpload};
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
    pending_skins: Vec<SkinUpload>,
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
        // The skin variants of the old library can name other textures in the new one.
        state.pending_skins.clear();
    }
    if let Some(mut skins) = main.get_resource_mut::<SkinSlots>() {
        let uploads = skins.take_uploads();
        state.pending_skins.extend(uploads);
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
    let Some(pyramid) = terrain.depth_pyramid() else {
        return;
    };
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
                depth: CORE_3D_DEPTH_FORMAT,
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
    let state = &mut *state;
    if let Some(gpu) = state.gpu.as_ref() {
        for skin in state.pending_skins.drain(..) {
            gpu.write_texels(&queue, skin.variant, skin.image.rgba());
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
    view: Option<Res<'w, FrameView>>,
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
    depth: ViewQuery<&ViewDepthStencilTexture>,
    mut state: ResMut<EntityRenderState>,
    params: EntityFrameParams,
    mut ctx: RenderContext,
) {
    let (Some(frame), Some(terrain), Some(view)) = (
        params.frame.as_deref(),
        params.terrain.as_deref(),
        params.view.as_deref(),
    ) else {
        return;
    };
    let depth = depth.into_inner();
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
    let light = params
        .shade
        .map_or_else(CardinalLight::default, |shade| shade.0);
    let mut uniform = FrameUniform::new(view, clock.tick, clock.partial_tick, light);
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
            view: depth.attachment.depth_stencil_views().attachment_view(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::{ScatterEntry, TickRecord};

    fn entry(slot: u32, tick: u32, x: i32) -> ScatterEntry {
        ScatterEntry {
            slot,
            tick,
            record: TickRecord {
                block: [x, 0, 0],
                ..TickRecord::default()
            },
        }
    }

    #[test]
    fn a_lagging_upload_keeps_the_last_tick_record_of_a_slot_and_its_fresh_bit() {
        let mut pending = SlotUpload {
            ticks: vec![entry(3, 5 | SCATTER_FRESH, 1)],
            ..SlotUpload::default()
        };
        let next = SlotUpload {
            slot_capacity: 64,
            slot_count: 5,
            ticks: vec![entry(3, 6, 2), entry(4, 6, 7)],
            ..SlotUpload::default()
        };
        merge(&mut pending, next);
        assert_eq!(
            pending.ticks,
            vec![entry(3, 6 | SCATTER_FRESH, 2), entry(4, 6, 7)]
        );
        assert_eq!((pending.slot_capacity, pending.slot_count), (64, 5));
    }
}
