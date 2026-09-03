use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::terrain::Terrain;
use super::texture::{array_view, atlas_sampler, create_lightmap, create_tints, upload_atlases};
use super::{Animation, Atlas, Budget};

const TICKS_PER_SECOND: f64 = 20.0;

#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct AnimationFrame {
    layer: u32,
    next: u32,
    blend: f32,
    _pad: u32,
}

const UNWRITTEN: AnimationFrame = AnimationFrame {
    layer: u32::MAX,
    next: u32::MAX,
    blend: 0.0,
    _pad: 0,
};

impl Animation {
    fn at(&self, ticks: f64) -> AnimationFrame {
        let count = self.count.max(1);
        let elapsed = ticks / f64::from(self.frametime.max(1));
        let step = (elapsed as u64 % u64::from(count)) as u32;
        AnimationFrame {
            layer: self.base_layer + step,
            next: self.base_layer + (step + 1) % count,
            blend: if self.interpolate != 0 {
                elapsed.fract() as f32
            } else {
                0.0
            },
            _pad: 0,
        }
    }
}

pub(super) struct Sprites {
    pub atlases: Vec<TextureView>,
    pub atlas_sampler: Sampler,
    pub tints: Texture,
    pub tints_view: TextureView,
    pub tint_sampler: Sampler,
    pub lightmap: Texture,
    pub lightmap_view: TextureView,
    pub frames: Buffer,
    pub animated_from: u32,
    animations: Vec<Animation>,
    written: Vec<AnimationFrame>,
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
            frames: frame_buffer(0, device),
            animated_from: 0,
            animations: Vec::new(),
            written: Vec::new(),
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
        self.frames = frame_buffer(animations.len(), device);
        self.animated_from = animated_from;
        self.animations = animations.to_vec();
        self.written = vec![UNWRITTEN; animations.len()];
        spent
    }
}

/// Steps every animation on the CPU once a frame, so a fragment reads its two layers and a
/// blend instead of dividing and taking modulos of the clock.
pub(super) fn write_animation_frames(
    terrain: Option<ResMut<Terrain>>,
    time: Res<Time>,
    queue: Res<RenderQueue>,
) {
    let Some(mut terrain) = terrain else {
        return;
    };
    let sprites = &mut terrain.sprites;
    let ticks = time.elapsed_secs_f64() * TICKS_PER_SECOND;
    let mut changed = false;
    for (animation, written) in sprites.animations.iter().zip(&mut sprites.written) {
        let frame = animation.at(ticks);
        changed |= *written != frame;
        *written = frame;
    }
    if changed {
        queue.write_buffer(&sprites.frames, 0, bytemuck::cast_slice(&sprites.written));
    }
}

fn frame_buffer(animations: usize, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("terrain animation frames"),
        size: (animations.max(1) * size_of::<AnimationFrame>()) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
