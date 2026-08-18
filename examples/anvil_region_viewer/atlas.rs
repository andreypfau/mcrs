//! Collects the sprites a region actually references into one `texture_2d_array`.
//!
//! An array rather than an atlas: a greedy-merged quad spanning `w × h` blocks gets UVs `0..w, 0..h`
//! and `AddressMode::Repeat` tiles it for free. An atlas would need `fract()` in the fragment shader
//! and would still bleed neighbouring sprites into the lower mips.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

use crate::model;

/// How a sprite's alpha channel decides which pass its geometry belongs to.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Opacity {
    /// Every texel is fully opaque.
    Solid,
    /// Texels are either fully opaque or fully transparent — an alpha-tested `discard`.
    Cutout,
    /// At least one texel is partially transparent — needs blending.
    Translucent,
}

pub struct Sprite {
    pub opacity: Opacity,
}

/// Sprites are square and all the same size, so the whole set is one array texture. Animated
/// sprites are a vertical strip of frames in the source PNG; only the first frame is taken.
pub struct SpriteRegistry {
    pub sprites: Vec<Sprite>,
    index: HashMap<String, u16>,
    /// Side length of every layer, chosen as the largest source sprite so nothing is downscaled.
    size: u32,
    /// RGBA8 pixels, layer-major, mip 0 only. Mips are derived at upload time.
    pixels: Vec<u8>,
    /// Decoded but not yet resized source frames, kept until [`Self::finish`] fixes the size.
    pending: Vec<(u32, Vec<u8>)>,
}

impl SpriteRegistry {
    pub fn new() -> Self {
        Self {
            sprites: Vec::new(),
            index: HashMap::new(),
            size: 0,
            pixels: Vec::new(),
            pending: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    /// Interns a sprite id, decoding its PNG the first time it is seen.
    pub fn intern(&mut self, id: &str) -> Result<u16, String> {
        if let Some(&layer) = self.index.get(id) {
            return Ok(layer);
        }
        let path = model::resource_path(id, "textures", "png");
        let bytes =
            std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let image = Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
        let width = image.width();
        let data = image
            .data
            .ok_or_else(|| format!("{} decoded without pixel data", path.display()))?;
        let frame = (width * width * 4) as usize;
        if data.len() < frame {
            return Err(format!(
                "{} is smaller than one square frame",
                path.display()
            ));
        }
        let frame = data[..frame].to_vec();

        let mut opaque = true;
        let mut binary = true;
        for i in (3..frame.len()).step_by(4) {
            let a = frame[i];
            if a != 255 {
                opaque = false;
            }
            if a != 255 && a != 0 {
                binary = false;
            }
        }
        let opacity = match (opaque, binary) {
            (true, _) => Opacity::Solid,
            (false, true) => Opacity::Cutout,
            (false, false) => Opacity::Translucent,
        };

        let layer = self.sprites.len() as u16;
        self.sprites.push(Sprite { opacity });
        self.index.insert(id.to_string(), layer);
        self.size = self.size.max(width);
        self.pending.push((width, frame));
        Ok(layer)
    }

    /// Resamples every sprite to the common layer size. Called once, after all sprites are interned.
    pub fn finish(&mut self) {
        if self.size == 0 {
            self.size = 16;
        }
        let size = self.size as usize;
        self.pixels = vec![0; size * size * 4 * self.pending.len().max(1)];
        for (layer, (width, frame)) in self.pending.iter().enumerate() {
            let dst = &mut self.pixels[layer * size * size * 4..(layer + 1) * size * size * 4];
            let src_size = *width as usize;
            // Nearest-neighbour upscale keeps the pixel-art edges crisp; sources are never larger
            // than `size`, so this never has to average texels down.
            for y in 0..size {
                let sy = y * src_size / size;
                for x in 0..size {
                    let sx = x * src_size / size;
                    let s = (sy * src_size + sx) * 4;
                    let d = (y * size + x) * 4;
                    dst[d..d + 4].copy_from_slice(&frame[s..s + 4]);
                }
            }
        }
        self.pending.clear();
        self.pending.shrink_to_fit();
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    /// Mip 0 followed by every smaller level, layer-major within each level. Each level is laid out
    /// exactly as `write_texture` wants it, so the caller uploads one level per call.
    pub fn mip_chain(&self) -> Vec<Vec<u8>> {
        let layers = self.sprites.len().max(1);
        let mut levels = vec![self.pixels.clone()];
        let mut size = self.size as usize;
        while size > 1 {
            let half = size / 2;
            let previous = levels.last().unwrap();
            let mut level = vec![0u8; half * half * 4 * layers];
            for layer in 0..layers {
                let src = &previous[layer * size * size * 4..(layer + 1) * size * size * 4];
                let dst = &mut level[layer * half * half * 4..(layer + 1) * half * half * 4];
                for y in 0..half {
                    for x in 0..half {
                        downsample_2x2(src, size, x * 2, y * 2, &mut dst[(y * half + x) * 4..]);
                    }
                }
            }
            levels.push(level);
            size = half;
        }
        levels
    }
}

/// Alpha-weighted so a cutout sprite does not bleed the colour of its fully transparent texels —
/// those carry arbitrary RGB in the vanilla pack and a plain average turns leaf edges black.
fn downsample_2x2(src: &[u8], stride: usize, x: usize, y: usize, dst: &mut [u8]) {
    let mut rgb = [0u32; 3];
    let mut alpha = 0u32;
    let mut weight = 0u32;
    for dy in 0..2 {
        for dx in 0..2 {
            let s = ((y + dy) * stride + x + dx) * 4;
            let a = src[s + 3] as u32;
            for c in 0..3 {
                rgb[c] += src[s + c] as u32 * a;
            }
            alpha += a;
            weight += a;
        }
    }
    if weight == 0 {
        dst[..4].copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    for c in 0..3 {
        dst[c] = (rgb[c] / weight) as u8;
    }
    dst[3] = (alpha / 4) as u8;
}

#[cfg(test)]
mod tests {
    use super::downsample_2x2;

    #[test]
    fn downsampling_ignores_the_colour_of_transparent_texels() {
        // Three transparent black texels beside one opaque white one must stay white, not go grey.
        let src = [
            255, 255, 255, 255, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut dst = [0u8; 4];
        downsample_2x2(&src, 2, 0, 0, &mut dst);
        assert_eq!(&dst[..3], &[255, 255, 255]);
        assert_eq!(dst[3], 63);
    }
}
