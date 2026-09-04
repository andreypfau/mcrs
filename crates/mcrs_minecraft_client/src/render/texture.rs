use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use crate::blocks::TINT_KINDS;
use crate::sky::SkyUniform;

use super::{AtlasUpdate, Budget};

const TINT_LAYERS: u32 = TINT_KINDS as u32;

const LIGHT_LEVELS: u32 = 16;

/// One texture array per sprite size, holding stills from layer zero up and animation frames
/// from the top layer down, so a layer written once never moves; only a regrow copies.
pub(super) struct AtlasSlot {
    pub texture: Texture,
    pub view: TextureView,
    pub size: u32,
    pub capacity: u32,
    pub stills: u32,
    pub frames: u32,
}

const FIRST_CAPACITY: u32 = 64;

/// A buffer-to-texture copy wants rows on 256-byte boundaries.
const ROW_ALIGNMENT: u32 = 256;

fn atlas_texture(index: usize, size: u32, capacity: u32, device: &RenderDevice) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(&format!("terrain atlas {index}")),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: capacity,
        },
        mip_level_count: size.trailing_zeros() + 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

pub(super) fn blank_atlas(index: usize, device: &RenderDevice) -> AtlasSlot {
    let texture = atlas_texture(index, 1, 1, device);
    AtlasSlot {
        view: array_view(&texture),
        texture,
        size: 1,
        capacity: 1,
        stills: 0,
        frames: 0,
    }
}

/// Where staged layers wait for their copy into the atlas: belt memory cannot be the source of a
/// texture copy directly, so the update goes belt, then here, then into the array.
pub(super) fn atlas_staging(bytes: u64, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("terrain atlas staging"),
        size: bytes.max(ROW_ALIGNMENT as u64),
        usage: BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn pitch(width: u32) -> u32 {
    (width * 4).div_ceil(ROW_ALIGNMENT) * ROW_ALIGNMENT
}

/// Bytes a set of levels takes in the staging layout.
fn staged_bytes(size: u32, levels: usize, layers: u32) -> u64 {
    (0..levels)
        .map(|level| {
            let side = (size >> level).max(1);
            u64::from(pitch(side)) * u64::from(side) * u64::from(layers)
        })
        .sum()
}

pub(super) struct AtlasWriter<'a> {
    pub device: &'a RenderDevice,
    pub encoder: &'a mut CommandEncoder,
    pub belt: &'a mut wgpu::util::StagingBelt,
    pub staging: &'a mut Buffer,
    pub used: u64,
}

impl AtlasWriter<'_> {
    /// Grows the slot to fit, carrying what it already holds, and writes the new layers. Returns
    /// whether the view changed, which is when the bind groups have to follow.
    pub fn apply(&mut self, index: usize, slot: &mut AtlasSlot, update: &AtlasUpdate) -> bool {
        let needed = update.stills + update.frames;
        let mut rebound = false;
        if slot.size != update.size || needed > slot.capacity {
            let limit = self.device.limits().max_texture_array_layers;
            assert!(
                needed <= limit,
                "{needed} sprites are {0}x{0}, but this device binds at most {limit} array layers",
                update.size,
            );
            let capacity = needed.max(FIRST_CAPACITY).next_power_of_two().min(limit);
            let texture = atlas_texture(index, update.size, capacity, self.device);
            if slot.size == update.size {
                self.carry(slot, &texture, capacity);
            } else {
                slot.stills = 0;
                slot.frames = 0;
            }
            slot.view = array_view(&texture);
            slot.texture = texture;
            slot.capacity = capacity;
            slot.size = update.size;
            rebound = true;
        }
        debug_assert!(update.first_still <= slot.stills && update.first_frame <= slot.frames);
        if update.stills > update.first_still {
            let layers = update.stills - update.first_still;
            self.write(slot, &update.still_mips, update.first_still, layers, false);
        }
        if update.frames > update.first_frame {
            let layers = update.frames - update.first_frame;
            let lowest = slot.capacity - update.frames;
            self.write(slot, &update.frame_mips, lowest, layers, true);
        }
        slot.stills = update.stills;
        slot.frames = update.frames;
        rebound
    }

    fn carry(&mut self, slot: &AtlasSlot, texture: &Texture, capacity: u32) {
        let levels = slot.size.trailing_zeros() + 1;
        for level in 0..levels {
            let side = (slot.size >> level).max(1);
            for (from, to, count) in [
                (0, 0, slot.stills),
                (
                    slot.capacity - slot.frames,
                    capacity - slot.frames,
                    slot.frames,
                ),
            ] {
                if count == 0 {
                    continue;
                }
                self.encoder.copy_texture_to_texture(
                    TexelCopyTextureInfo {
                        texture: &slot.texture,
                        mip_level: level,
                        origin: Origin3d {
                            x: 0,
                            y: 0,
                            z: from,
                        },
                        aspect: TextureAspect::All,
                    },
                    TexelCopyTextureInfo {
                        texture,
                        mip_level: level,
                        origin: Origin3d { x: 0, y: 0, z: to },
                        aspect: TextureAspect::All,
                    },
                    Extent3d {
                        width: side,
                        height: side,
                        depth_or_array_layers: count,
                    },
                );
            }
        }
    }

    /// Frames are given lowest source index first but live highest layer first, so their block
    /// is written back to front.
    fn write(
        &mut self,
        slot: &AtlasSlot,
        mips: &[Vec<u8>],
        first_layer: u32,
        layers: u32,
        reversed: bool,
    ) {
        let bytes = staged_bytes(slot.size, mips.len(), layers);
        if self.used + bytes > self.staging.size() {
            *self.staging = atlas_staging((self.used + bytes).next_power_of_two(), self.device);
            self.used = 0;
        }
        for (level, data) in mips.iter().enumerate() {
            let side = (slot.size >> level).max(1);
            let row = (side * 4) as usize;
            let pitch = pitch(side);
            let image = pitch as usize * side as usize;
            let size = image as u64 * u64::from(layers);
            let offset = self.used;
            let mut view = self.belt.write_buffer(
                self.encoder,
                self.staging,
                offset,
                BufferSize::new(size).expect("at least one layer"),
            );
            for layer in 0..layers as usize {
                let source = if reversed {
                    layers as usize - 1 - layer
                } else {
                    layer
                };
                let src = &data[source * row * side as usize..][..row * side as usize];
                for y in 0..side as usize {
                    let at = layer * image + y * pitch as usize;
                    view.slice(at..at + row)
                        .copy_from_slice(&src[y * row..][..row]);
                }
            }
            drop(view);
            self.encoder.copy_buffer_to_texture(
                TexelCopyBufferInfo {
                    buffer: self.staging,
                    layout: TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some(pitch),
                        rows_per_image: Some(side),
                    },
                },
                TexelCopyTextureInfo {
                    texture: &slot.texture,
                    mip_level: level as u32,
                    origin: Origin3d {
                        x: 0,
                        y: 0,
                        z: first_layer,
                    },
                    aspect: TextureAspect::All,
                },
                Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: layers,
                },
            );
            self.used += size;
        }
    }
}

pub(super) fn array_view(texture: &Texture) -> TextureView {
    texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    })
}

pub(super) fn atlas_sampler(device: &RenderDevice) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label: Some("terrain atlas"),
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: MipmapFilterMode::Linear,
        ..default()
    })
}

pub(super) fn create_tints(budget: &Budget, device: &RenderDevice) -> (Texture, Sampler) {
    let [width, height] = budget.tint_size;
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("terrain tints"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: TINT_LAYERS,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("terrain tints"),
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    (texture, sampler)
}

pub(super) fn write_tint_square(
    tints: &Texture,
    queue: &RenderQueue,
    origin: [u32; 2],
    size: u32,
    data: &[u8],
) {
    let layer = (size * size * 4) as usize;
    for kind in 0..TINT_LAYERS {
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: tints,
                mip_level: 0,
                origin: Origin3d {
                    x: origin[0],
                    y: origin[1],
                    z: kind,
                },
                aspect: TextureAspect::All,
            },
            &data[kind as usize * layer..(kind as usize + 1) * layer],
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
    }
}

pub(super) fn create_lightmap(device: &RenderDevice) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("terrain lightmap"),
        size: Extent3d {
            width: LIGHT_LEVELS,
            height: LIGHT_LEVELS,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba32Float,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

pub(super) fn write_lightmap(lightmap: &Texture, queue: &RenderQueue, sky: &SkyUniform) {
    let levels = LIGHT_LEVELS as usize;
    let mut texels = [[0.0f32; 4]; (LIGHT_LEVELS * LIGHT_LEVELS) as usize];
    for sky_level in 0..levels {
        for block_level in 0..levels {
            texels[sky_level * levels + block_level] =
                lit_color(sky, block_level as f32, sky_level as f32);
        }
    }
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: lightmap,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        bytemuck::cast_slice(&texels),
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(LIGHT_LEVELS * size_of::<[f32; 4]>() as u32),
            rows_per_image: Some(LIGHT_LEVELS),
        },
        Extent3d {
            width: LIGHT_LEVELS,
            height: LIGHT_LEVELS,
            depth_or_array_layers: 1,
        },
    );
}

fn light_curve(level: f32) -> f32 {
    let f = level / 15.0;
    f / (4.0 - 3.0 * f)
}

fn lit_color(sky: &SkyUniform, block_level: f32, sky_level: f32) -> [f32; 4] {
    let rgb = |channels: [f32; 4]| Vec3::from_slice(&channels);
    let mut color = rgb(sky.ambient);
    color += rgb(sky.sky_light) * light_curve(sky_level) * sky.sky_light[3];
    let f = block_level / 15.0;
    let parabolic = (2.0 * f - 1.0) * (2.0 * f - 1.0);
    let tint = rgb(sky.block_light).lerp(Vec3::ONE, 0.9 * parabolic);
    color += tint * light_curve(block_level) * sky.block_light[3];
    color.clamp(Vec3::ZERO, Vec3::ONE).extend(1.0).to_array()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sky() -> SkyUniform {
        SkyUniform {
            ambient: [0.1, 0.1, 0.1, 1.0],
            sky_light: [0.2, 0.4, 0.6, 0.5],
            block_light: [1.0, 0.5, 0.0, 0.8],
            ..default()
        }
    }

    fn assert_lit(block_level: f32, sky_level: f32, expected: [f32; 3]) {
        let got = lit_color(&sky(), block_level, sky_level);
        let close = (0..3).all(|c| (got[c] - expected[c]).abs() < 1e-6);
        assert!(
            close,
            "({block_level}, {sky_level}) lit to {got:?}, want {expected:?}"
        );
        assert_eq!(got[3], 1.0);
    }

    #[test]
    fn the_light_curve_holds_its_shape() {
        assert_eq!(light_curve(0.0), 0.0);
        assert_eq!(light_curve(15.0), 1.0);
        assert!((light_curve(5.0) - 1.0 / 9.0).abs() < 1e-6);
        assert!((light_curve(10.0) - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn the_lightmap_matches_the_curve_worked_by_hand() {
        assert_lit(0.0, 15.0, [0.2, 0.3, 0.4]);
        assert_lit(15.0, 0.0, [0.9, 0.86, 0.82]);
        assert_lit(5.0, 10.0, [0.222_222_2, 0.215_555_5, 0.208_888_9]);
        assert_lit(15.0, 15.0, [1.0, 1.0, 1.0]);
    }
}
