use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::texture::{array_view, atlas_sampler, create_lightmap, create_tints, upload_atlases};
use super::{Animation, Atlas, Budget};

pub(super) struct Sprites {
    pub atlases: Vec<TextureView>,
    pub atlas_sampler: Sampler,
    pub tints: Texture,
    pub tints_view: TextureView,
    pub tint_sampler: Sampler,
    pub lightmap: Texture,
    pub lightmap_view: TextureView,
    pub animations: Buffer,
    pub animated_from: u32,
}

impl Sprites {
    pub fn new(budget: &Budget, device: &RenderDevice, queue: &RenderQueue) -> Self {
        let (tints, tint_sampler) = create_tints(budget, device);
        let lightmap = create_lightmap(device);
        Self {
            atlases: upload_atlases(&[], device, queue),
            atlas_sampler: atlas_sampler(device),
            tints_view: array_view(&tints),
            tints,
            tint_sampler,
            lightmap_view: lightmap.create_view(&TextureViewDescriptor::default()),
            lightmap,
            animations: animation_buffer(&[], device),
            animated_from: 0,
        }
    }

    pub fn swap(
        &mut self,
        atlases: Vec<Atlas>,
        animations: &[Animation],
        animated_from: u32,
        device: &RenderDevice,
        queue: &RenderQueue,
    ) -> usize {
        let spent = atlases
            .iter()
            .flat_map(|atlas| atlas.mips.iter())
            .map(|mip| mip.len())
            .sum();
        self.atlases = upload_atlases(&atlases, device, queue);
        self.animations = animation_buffer(animations, device);
        self.animated_from = animated_from;
        spent
    }
}

fn animation_buffer(animations: &[Animation], device: &RenderDevice) -> Buffer {
    let padding = [Animation::default()];
    device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain animations"),
        contents: bytemuck::cast_slice(if animations.is_empty() {
            &padding[..]
        } else {
            animations
        }),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    })
}
