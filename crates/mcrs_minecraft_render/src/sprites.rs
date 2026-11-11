use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use super::render_path::RenderPath;
use super::terrain::Terrain;
use super::texture::{
    AtlasSlot, AtlasWriter, array_view, atlas_sampler, atlas_staging, blank_atlas, create_lightmap,
    create_tints,
};
use super::words::{Staged, Words};
use super::{Animation, Budget, PinnedTick, SpriteEntry, SpriteUpload};
use mcrs_minecraft_mesh::pack::{MAX_SPRITE_ARRAYS, MAX_SPRITES};

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
            layer: self.first_layer + step,
            next: self.first_layer + (step + 1) % count,
            blend: if self.interpolate != 0 {
                elapsed.fract() as f32
            } else {
                0.0
            },
            _pad: 0,
        }
    }
}

pub struct Sprites {
    pub atlases: [AtlasSlot; MAX_SPRITE_ARRAYS],
    pub atlas_sampler: Sampler,
    pub(crate) tints: Texture,
    pub(crate) tints_view: TextureView,
    pub(crate) tint_sampler: Sampler,
    pub(crate) lightmap: Texture,
    pub(crate) lightmap_view: TextureView,
    pub(crate) lightmap_sampler: Sampler,
    /// An [`AnimationFrame`] per animation, as words.
    pub frames: Words,
    /// A [`SpriteEntry`] per sprite, as words.
    pub table: Words,
    animations: Vec<Animation>,
    written: Vec<AnimationFrame>,
}

impl Sprites {
    pub(crate) fn new(path: RenderPath, budget: &Budget, device: &RenderDevice) -> Self {
        // Sized once for every sprite an id can name, so neither table ever moves.
        let table = |label, row: usize| Words::new(path, label, (MAX_SPRITES * row) as u64, device);
        let (tints, tint_sampler) = create_tints(budget, device);
        let (lightmap, lightmap_sampler) = create_lightmap(device);
        Self {
            atlases: std::array::from_fn(|index| blank_atlas(index, device)),
            atlas_sampler: atlas_sampler(device),
            tints_view: array_view(&tints),
            tints,
            tint_sampler,
            lightmap_view: lightmap.create_view(&TextureViewDescriptor::default()),
            lightmap,
            lightmap_sampler,
            frames: table("terrain animation frames", size_of::<AnimationFrame>()),
            table: table("terrain sprite table", size_of::<SpriteEntry>()),
            animations: Vec::new(),
            written: Vec::new(),
        }
    }

    /// Takes the layers and sprites the bake added and the animation table as it now stands.
    /// Returns the bytes staged and whether a texture was replaced, which is when the bind
    /// groups follow.
    pub(crate) fn update(
        &mut self,
        upload: &SpriteUpload,
        device: &RenderDevice,
        staged: &mut Staged,
    ) -> (usize, bool) {
        // Uploads come in a burst at load and rarely after, so the buffer is not kept between
        // them: holding the largest one ever made would pin tens of megabytes for good.
        let mut staging = atlas_staging(FIRST_STAGING_BYTES, device);
        let mut writer = AtlasWriter {
            device,
            encoder: &mut *staged.encoder,
            belt: &mut *staged.belt,
            staging: &mut staging,
            used: 0,
        };
        let SpriteUpload {
            atlases,
            table_from,
            table,
            animations,
        } = upload;
        let mut rebound = false;
        for (index, (slot, update)) in self.atlases.iter_mut().zip(atlases).enumerate() {
            rebound |= writer.apply(index, slot, update);
        }
        let mut spent = writer.used as usize;
        let bytes: &[u8] = bytemuck::cast_slice(table);
        let offset = u64::from(*table_from) * size_of::<SpriteEntry>() as u64;
        self.table.stage(offset, bytes, staged);
        spent += bytes.len();
        info!(
            layers = ?atlases.iter().map(|u| (u.size, u.layers - u.first)).collect::<Vec<_>>(),
            sprites = table.len(),
            animations = animations.len(),
            bytes = spent,
            rebound,
            "added sprites"
        );
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
    pinned: Option<Res<PinnedTick>>,
    queue: Res<RenderQueue>,
) {
    let Some(mut terrain) = terrain else {
        return;
    };
    let sprites = &mut terrain.sprites;
    let ticks = match pinned {
        Some(pinned) => pinned.0 as f64,
        None => time.elapsed_secs_f64() * TICKS_PER_SECOND,
    };
    let mut changed = false;
    for (animation, written) in sprites.animations.iter().zip(&mut sprites.written) {
        let frame = animation.at(ticks);
        changed |= *written != frame;
        *written = frame;
    }
    if changed {
        let frames = bytemuck::cast_slice(&sprites.written);
        sprites.frames.write(0, frames, &queue);
    }
}
