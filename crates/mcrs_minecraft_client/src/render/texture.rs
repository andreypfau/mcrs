use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use crate::pack::MAX_SPRITE_ARRAYS;
use crate::sky::SkyUniform;

use super::{Atlas, Budget};

pub(super) const TINT_LAYERS: u32 = 3;

pub(super) const LIGHT_LEVELS: u32 = 16;

pub(super) fn upload_atlases(
    atlases: &[Atlas],
    device: &RenderDevice,
    queue: &RenderQueue,
) -> (Vec<TextureView>, Sampler) {
    let limit = device.limits().max_texture_array_layers;
    let blank = Atlas {
        size: 1,
        layers: 1,
        mips: vec![vec![0u8; 4]],
    };
    let views = (0..MAX_SPRITE_ARRAYS)
        .map(|index| {
            let atlas = atlases.get(index).unwrap_or(&blank);
            assert!(
                atlas.layers <= limit,
                "{} sprites are {}x{}, but this device binds at most {limit} array layers",
                atlas.layers,
                atlas.size,
                atlas.size,
            );
            upload_atlas(atlas, &format!("terrain atlas {index}"), device, queue)
        })
        .collect();
    (views, atlas_sampler(device))
}

pub(super) fn upload_atlas(
    atlas: &Atlas,
    label: &str,
    device: &RenderDevice,
    queue: &RenderQueue,
) -> TextureView {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: atlas.size,
            height: atlas.size,
            depth_or_array_layers: atlas.layers,
        },
        mip_level_count: atlas.mips.len() as u32,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, data) in atlas.mips.iter().enumerate() {
        let size = (atlas.size >> level).max(1);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: atlas.layers,
            },
        );
    }
    array_view(&texture)
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
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
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
    let mut texels = vec![[0.0f32; 4]; levels * levels];
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
        assert!(close, "({block_level}, {sky_level}) lit to {got:?}, want {expected:?}");
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
