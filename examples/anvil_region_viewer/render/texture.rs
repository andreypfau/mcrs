use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};

use crate::pack::MAX_SPRITE_ARRAYS;

use super::{Atlas, Layout};

pub(super) const TINT_LAYERS: u32 = 3;

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

pub(super) fn create_tints(layout: &Layout, device: &RenderDevice) -> (Texture, Sampler) {
    let [width, height] = layout.tint_size;
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
