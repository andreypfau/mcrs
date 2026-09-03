use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::terrain::Terrain;
use super::texture::{
    AtlasSlot, AtlasWriter, array_view, atlas_sampler, atlas_staging, blank_atlas, create_lightmap,
    create_tints,
};
use super::{Animation, AtlasUpdate, Budget};
use crate::pack::{MAX_SPRITE_ARRAYS, MAX_SPRITES};

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
    /// Frames sit under the top of their array, first frame highest, so the step counts down.
    fn at(&self, ticks: f64, capacity: u32) -> AnimationFrame {
        let count = self.count.max(1);
        let elapsed = ticks / f64::from(self.frametime.max(1));
        let step = (elapsed as u64 % u64::from(count)) as u32;
        let top = capacity - 1 - self.frame_base;
        AnimationFrame {
            layer: top - step,
            next: top - (step + 1) % count,
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
    pub atlases: Vec<AtlasSlot>,
    staging: Buffer,
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
    pub fn new(budget: &Budget, device: &RenderDevice) -> Self {
        let (tints, tint_sampler) = create_tints(budget, device);
        let lightmap = create_lightmap(device);
        Self {
            atlases: (0..MAX_SPRITE_ARRAYS)
                .map(|index| blank_atlas(index, device))
                .collect(),
            staging: atlas_staging(FIRST_STAGING_BYTES, device),
            atlas_sampler: atlas_sampler(device),
            tints_view: array_view(&tints),
            tints,
            tint_sampler,
            lightmap_view: lightmap.create_view(&TextureViewDescriptor::default()),
            lightmap,
            frames: frame_buffer(device),
            animated_from: 0,
            animations: Vec::new(),
            written: Vec::new(),
        }
    }

    /// Takes the layers the bake added and the animation table as it now stands. Returns the
    /// bytes staged and whether a texture was replaced, which is when the bind groups follow.
    pub fn update(
        &mut self,
        updates: &[AtlasUpdate],
        animations: &[Animation],
        animated_from: u32,
        device: &RenderDevice,
        encoder: &mut CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
    ) -> (usize, bool) {
        let mut writer = AtlasWriter {
            device,
            encoder,
            belt,
            staging: &mut self.staging,
            used: 0,
        };
        let mut rebound = false;
        for (index, update) in updates.iter().enumerate() {
            rebound |= writer.apply(index, &mut self.atlases[index], update);
        }
        let spent = writer.used as usize;
        info!(
            layers = ?updates.iter().map(|u| (u.size, u.stills - u.first_still, u.frames - u.first_frame)).collect::<Vec<_>>(),
            bytes = spent,
            rebound,
            "added sprite layers"
        );
        self.animated_from = animated_from;
        self.animations = animations.to_vec();
        self.written = vec![UNWRITTEN; animations.len()];
        (spent, rebound)
    }
}

const FIRST_STAGING_BYTES: u64 = 1 << 20;

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
        let frame = animation.at(ticks, sprites.atlases[animation.array as usize].capacity);
        changed |= *written != frame;
        *written = frame;
    }
    if changed {
        queue.write_buffer(&sprites.frames, 0, bytemuck::cast_slice(&sprites.written));
    }
}

/// Sized once for every animation a sprite reference can name, so the table never moves.
fn frame_buffer(device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("terrain animation frames"),
        size: (MAX_SPRITES * size_of::<AnimationFrame>()) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
