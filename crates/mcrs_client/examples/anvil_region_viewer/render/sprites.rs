use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::texture::{create_tints, upload_atlases};
use super::{Animation, Atlas, Layout};

pub(super) struct Sprites {
    pub atlases: Vec<TextureView>,
    pub atlas_sampler: Sampler,
    pub tints: Texture,
    pub tint_sampler: Sampler,
    pub animations: Buffer,
    pub animated_from: u32,
}

impl Sprites {
    pub fn new(layout: &Layout, device: &RenderDevice, queue: &RenderQueue) -> Self {
        let (atlases, atlas_sampler) = upload_atlases(&[], device, queue);
        let (tints, tint_sampler) = create_tints(layout, device);
        Self {
            atlases,
            atlas_sampler,
            tints,
            tint_sampler,
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
        let (views, sampler) = upload_atlases(&atlases, device, queue);
        self.atlases = views;
        self.atlas_sampler = sampler;
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
