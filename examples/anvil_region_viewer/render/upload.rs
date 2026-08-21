use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use crate::mesh::{Draw, Group};
use crate::pack::QUAD_WORDS;

use super::params;
use super::terrain::{Terrain, draw_bind_group};
use super::texture::{upload_atlases, write_tint_square};
use super::{Animation, Atlas};

static BUDGET: std::sync::LazyLock<usize> =
    std::sync::LazyLock::new(crate::config::upload_budget);

const COPY_ALIGN: usize = 4;

pub enum Upload {
    Tints {
        origin: [u32; 2],
        size: u32,
        data: Vec<u8>,
    },
    Sprites {
        atlases: Vec<Atlas>,
        animations: Vec<Animation>,
        animated_from: u32,
    },
    Geometry(Placement),
    Drop(u32),
}

pub struct Placement {
    pub quads: (u64, Vec<[u32; QUAD_WORDS]>),
    pub vertices: (u64, Vec<u32>),
    pub faces: (u64, Vec<u32>),
    pub groups: (u64, Vec<Group>),
    pub draws: Vec<Draw>,
}

#[derive(Resource, Clone, Default)]
pub struct Uploads(Arc<Mutex<Waiting>>);

#[derive(Default)]
pub struct Waiting {
    queue: VecDeque<Upload>,
    rebase: Option<Vec<(u32, u32)>>,
}

impl Uploads {
    pub fn push(&self, upload: Upload) {
        self.0.lock().unwrap().queue.push_back(upload);
    }

    pub fn rebase(&self, bases: Vec<(u32, u32)>) {
        self.0.lock().unwrap().rebase = Some(bases);
    }

    pub fn waiting(&self) -> usize {
        self.0.lock().unwrap().queue.len()
    }
}

pub(super) struct Pending {
    placement: Placement,
    part: usize,
    done: usize,
}

#[derive(Copy, Clone)]
enum Arena {
    Quads,
    Vertices,
    Faces,
    Groups,
}

const ARENA_PARTS: usize = 4;

impl Placement {
    fn part(&self, index: usize) -> (Arena, u64, &[u8]) {
        match index {
            0 => (Arena::Quads, self.quads.0, bytemuck::cast_slice(&self.quads.1)),
            1 => (
                Arena::Vertices,
                self.vertices.0,
                bytemuck::cast_slice(&self.vertices.1),
            ),
            2 => (
                Arena::Faces,
                self.faces.0,
                bytemuck::cast_slice(&self.faces.1),
            ),
            _ => (
                Arena::Groups,
                self.groups.0,
                bytemuck::cast_slice(&self.groups.1),
            ),
        }
    }
}

impl Terrain {
    fn arena(&self, arena: Arena) -> &Buffer {
        match arena {
            Arena::Quads => &self.quads,
            Arena::Vertices => &self.vertices,
            Arena::Faces => &self.faces,
            Arena::Groups => &self.group_buffer,
        }
    }
}

pub(super) fn apply_uploads(
    mut terrain: Option<ResMut<Terrain>>,
    uploads: Res<Uploads>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    let mut budget = *BUDGET;

    if let Some(bases) = uploads.0.lock().unwrap().rebase.take() {
        for (region, base) in bases {
            for draw in terrain.draws.iter_mut().filter(|draw| draw.region == region) {
                draw.cave_base = base;
            }
        }
        params::rebuild(terrain);
    }

    loop {
        if terrain.pending.is_none() {
            let next = uploads.0.lock().unwrap().queue.pop_front();
            match next {
                None => break,
                Some(Upload::Tints { origin, size, data }) => {
                    write_tint_square(terrain, &queue, origin, size, &data);
                    budget = budget.saturating_sub(data.len());
                    if budget == 0 {
                        break;
                    }
                    continue;
                }
                Some(Upload::Sprites {
                    atlases,
                    animations,
                    animated_from,
                }) => {
                    budget = budget.saturating_sub(swap_sprites(
                        terrain,
                        &device,
                        &queue,
                        &pipeline_cache,
                        atlases,
                        &animations,
                        animated_from,
                    ));
                    if budget == 0 {
                        break;
                    }
                    continue;
                }
                Some(Upload::Drop(region)) => {
                    terrain.draws.retain(|draw| draw.region != region);
                    params::rebuild(terrain);
                    continue;
                }
                Some(Upload::Geometry(placement)) => {
                    terrain.pending = Some(Pending {
                        placement,
                        part: 0,
                        done: 0,
                    });
                }
            }
        }

        let mut pending = terrain.pending.take().expect("just filled");
        while budget > 0 && pending.part < ARENA_PARTS {
            let (arena, offset, data) = pending.placement.part(pending.part);
            if data.is_empty() {
                pending.part += 1;
                pending.done = 0;
                continue;
            }
            let left = data.len() - pending.done;
            let mut take = left.min(budget);
            if take < left {
                take -= take % COPY_ALIGN;
                if take == 0 {
                    break;
                }
            }
            queue.write_buffer(
                terrain.arena(arena),
                offset + pending.done as u64,
                &data[pending.done..pending.done + take],
            );
            budget -= take;
            pending.done += take;
            if pending.done == data.len() {
                pending.part += 1;
                pending.done = 0;
            }
        }
        if pending.part < ARENA_PARTS {
            terrain.pending = Some(pending);
            break;
        }
        publish(terrain, pending.placement.draws);
        if budget == 0 {
            break;
        }
    }

    if terrain.params_dirty {
        params::write(terrain, &queue);
    }
}

fn swap_sprites(
    terrain: &mut Terrain,
    device: &RenderDevice,
    queue: &RenderQueue,
    pipeline_cache: &PipelineCache,
    atlases: Vec<Atlas>,
    animations: &[Animation],
    animated_from: u32,
) -> usize {
    let padding = [Animation::default()];
    let spent: usize = atlases
        .iter()
        .flat_map(|atlas| atlas.mips.iter())
        .map(|mip| mip.len())
        .sum();
    let (views, atlas_sampler) = upload_atlases(&atlases, device, queue);
    terrain.atlas_sampler = atlas_sampler;
    terrain.animations = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain animations"),
        contents: bytemuck::cast_slice(if animations.is_empty() {
            &padding[..]
        } else {
            animations
        }),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });
    terrain.draw_bind_group = draw_bind_group(
        device,
        pipeline_cache,
        &terrain.draw_layout,
        &terrain.quads,
        &terrain.vertices,
        &terrain.visible,
        &views,
        &terrain.atlas_sampler,
        &terrain.tints,
        &terrain.tint_sampler,
        &terrain.animations,
        &terrain.faces,
    );
    terrain.animated_from = animated_from;
    params::rebuild(terrain);
    spent
}

fn publish(terrain: &mut Terrain, draws: Vec<Draw>) {
    terrain.draws.extend(draws);
    terrain.draws.sort_by_key(|draw| draw.stream);
    params::rebuild(terrain);
}
