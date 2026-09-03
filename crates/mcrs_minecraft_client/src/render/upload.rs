use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use wgpu::util::StagingBelt;

use crate::mesh::{Draw, Group};
use crate::pack::QUAD_WORDS;

use super::arenas::Arenas;
use super::stats::FrameCounts;
use super::terrain::Terrain;
use super::texture::write_tint_square;
use super::{Animation, AtlasUpdate, SectionDesc};

static BUDGET: std::sync::LazyLock<usize> = std::sync::LazyLock::new(crate::config::upload_budget);

pub enum Upload {
    Tints {
        origin: [u32; 2],
        size: u32,
        data: Vec<u8>,
    },
    Sprites {
        atlases: Vec<AtlasUpdate>,
        animations: Vec<Animation>,
        animated_from: u32,
    },
    Geometry(Placement),
}

#[derive(Default)]
pub struct Placement {
    pub quads: (u64, Vec<[u32; QUAD_WORDS]>),
    pub vertices: (u64, Vec<u32>),
    pub faces: (u64, Vec<u32>),
    pub sections: (u64, Vec<SectionDesc>),
    pub groups: (u64, Vec<Group>),
    /// A whole new draw list, swapped in only once its group block has landed, so a draw never
    /// spends a frame pointing at a block that is still being written.
    pub draws: Option<Vec<Draw>>,
}

#[derive(Resource, Clone, Default)]
pub struct Uploads(Arc<Mutex<VecDeque<Upload>>>);

impl Uploads {
    pub fn push(&self, upload: Upload) {
        self.0.lock().unwrap().push_back(upload);
    }

    pub fn waiting(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

/// Geometry goes to the arenas through mapped staging memory the belt hands back after each
/// submit; `Queue::write_buffer` allocates a fresh staging buffer per call, which at the upload
/// budget costs a millisecond per megabyte.
#[derive(Resource)]
pub(super) struct Staging(Mutex<StagingBelt>);

impl Staging {
    pub fn new(device: &RenderDevice) -> Self {
        Self(Mutex::new(StagingBelt::new(
            device.wgpu_device().clone(),
            *BUDGET as u64,
        )))
    }
}

pub(super) fn recall_staging(staging: Option<Res<Staging>>) {
    if let Some(staging) = staging {
        staging.0.lock().unwrap().recall();
    }
}

pub(super) struct Pending {
    placement: Placement,
    part: usize,
    done: usize,
}

impl Placement {
    fn parts<'a>(&'a self, arenas: &'a Arenas) -> [(&'a Buffer, u64, &'a [u8]); 5] {
        [
            (
                &arenas.quads,
                self.quads.0,
                bytemuck::cast_slice(&self.quads.1),
            ),
            (
                &arenas.vertices,
                self.vertices.0,
                bytemuck::cast_slice(&self.vertices.1),
            ),
            (
                &arenas.faces,
                self.faces.0,
                bytemuck::cast_slice(&self.faces.1),
            ),
            (
                &arenas.sections,
                self.sections.0,
                bytemuck::cast_slice(&self.sections.1),
            ),
            (
                &arenas.groups,
                self.groups.0,
                bytemuck::cast_slice(&self.groups.1),
            ),
        ]
    }
}

#[derive(SystemParam)]
pub(super) struct UploadParams<'w> {
    pub terrain: Option<ResMut<'w, Terrain>>,
    staging: Option<Res<'w, Staging>>,
    uploads: Res<'w, Uploads>,
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    pipeline_cache: Res<'w, PipelineCache>,
    counts: Res<'w, FrameCounts>,
}

pub(super) fn apply_uploads(params: &mut UploadParams, encoder: &mut CommandEncoder) {
    let UploadParams {
        terrain,
        staging,
        uploads,
        device,
        queue,
        pipeline_cache,
        counts,
    } = params;
    let (Some(terrain), Some(staging)) = (terrain.as_mut(), staging) else {
        return;
    };
    let terrain = terrain.as_mut();
    let mut belt = staging.0.lock().unwrap();
    let mut budget = *BUDGET;

    while budget > 0 {
        if terrain.arenas.pending.is_none() {
            let next = uploads.0.lock().unwrap().pop_front();
            match next {
                None => break,
                Some(Upload::Tints { origin, size, data }) => {
                    let _writing = info_span!("upload tints").entered();
                    write_tint_square(&terrain.sprites.tints, queue, origin, size, &data);
                    budget = budget.saturating_sub(data.len());
                    continue;
                }
                Some(Upload::Sprites {
                    atlases,
                    animations,
                    animated_from,
                }) => {
                    let _adding = info_span!("upload sprites").entered();
                    let (spent, rebound) = terrain.sprites.update(
                        &atlases,
                        &animations,
                        animated_from,
                        device,
                        encoder,
                        &mut belt,
                    );
                    if rebound {
                        terrain.binds.rebuild_draw(
                            &terrain.arenas,
                            &terrain.sprites,
                            device,
                            pipeline_cache,
                        );
                    }
                    budget = budget.saturating_sub(spent);
                    continue;
                }
                Some(Upload::Geometry(placement)) => {
                    terrain.arenas.pending = Some(Pending {
                        placement,
                        part: 0,
                        done: 0,
                    });
                }
            }
        }

        let mut pending = terrain.arenas.pending.take().expect("just filled");
        let parts = pending.placement.parts(&terrain.arenas);
        let copying = info_span!("upload copy").entered();
        while budget > 0 && pending.part < parts.len() {
            let (buffer, offset, data) = parts[pending.part];
            if data.is_empty() {
                pending.part += 1;
                pending.done = 0;
                continue;
            }
            let left = data.len() - pending.done;
            let mut take = left.min(budget);
            if take < left {
                take -= take % COPY_BUFFER_ALIGNMENT as usize;
                if take == 0 {
                    break;
                }
            }
            belt.write_buffer(
                encoder,
                buffer,
                offset + pending.done as u64,
                BufferSize::new(take as u64).expect("a non-empty part"),
            )
            .copy_from_slice(&data[pending.done..pending.done + take]);
            budget -= take;
            pending.done += take;
            if pending.done == data.len() {
                pending.part += 1;
                pending.done = 0;
            }
        }
        drop(copying);
        if pending.part < parts.len() {
            terrain.arenas.pending = Some(pending);
            break;
        }
        if let Some(draws) = pending.placement.draws {
            terrain.list.draws = draws;
        }
        let _rebuilding = info_span!("upload rebuild params").entered();
        terrain.rebuild_params(device, pipeline_cache);
    }

    {
        let _finishing = info_span!("upload finish").entered();
        belt.finish();
    }
    counts.set_upload_bytes(*BUDGET - budget);
    let _flushing = info_span!("upload flush params").entered();
    terrain.list.flush(&terrain.frame.params, queue);
}
