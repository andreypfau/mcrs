//! Collects the sprites a region actually references into one `texture_2d_array` per resolution.
//!
//! An array rather than an atlas: a greedy-merged quad spanning `w × h` blocks gets UVs `0..w, 0..h`
//! and `AddressMode::Repeat` tiles it for free. An atlas would need `fract()` in the fragment shader
//! and would still bleed neighbouring sprites into the lower mips.
//!
//! One array per resolution rather than one for everything: every layer of an array is the same
//! size, so a single 512² texture in a resource pack would otherwise drag all eleven hundred 16²
//! ones up to its size, and the memory that costs grows with the square.

use std::collections::HashMap;
use std::sync::LazyLock;

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

/// Where a sprite lives: which array, and which layer of that array.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct SpriteRef {
    pub array: u8,
    pub layer: u16,
}

/// Every sprite of one resolution, which becomes one array texture. Sprites are square; animated
/// ones are a vertical strip of frames in the source PNG and only the first frame is taken.
pub struct SpriteArray {
    /// Side length of every layer.
    pub size: u32,
    pub sprites: Vec<Sprite>,
    /// RGBA8 pixels, layer-major, mip 0 only. Mips are derived at upload time.
    pixels: Vec<u8>,
}

pub struct SpriteRegistry {
    arrays: Vec<SpriteArray>,
    index: HashMap<String, SpriteRef>,
}

impl SpriteRegistry {
    pub fn new() -> Self {
        Self {
            arrays: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Every sprite of every resolution.
    pub fn len(&self) -> usize {
        self.arrays.iter().map(|array| array.sprites.len()).sum()
    }

    pub fn arrays(&self) -> &[SpriteArray] {
        &self.arrays
    }

    pub fn opacity(&self, sprite: SpriteRef) -> Opacity {
        self.arrays[sprite.array as usize].sprites[sprite.layer as usize].opacity
    }

    /// Interns a sprite id, decoding its PNG the first time it is seen.
    pub fn intern(&mut self, id: &str) -> Result<SpriteRef, String> {
        if let Some(&sprite) = self.index.get(id) {
            return Ok(sprite);
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

        let array = match self.arrays.iter().position(|array| array.size == width) {
            Some(array) => array,
            None => {
                self.arrays.push(SpriteArray {
                    size: width,
                    sprites: Vec::new(),
                    pixels: Vec::new(),
                });
                self.arrays.len() - 1
            }
        };
        let sprite = SpriteRef {
            array: array as u8,
            layer: self.arrays[array].sprites.len() as u16,
        };
        let array = &mut self.arrays[array];
        array.sprites.push(Sprite { opacity });
        array.pixels.extend_from_slice(&frame);
        self.index.insert(id.to_string(), sprite);
        Ok(sprite)
    }
}

impl SpriteArray {
    pub fn layers(&self) -> u32 {
        self.sprites.len().max(1) as u32
    }

    /// Mip 0 followed by every smaller level, layer-major within each level. Each level is laid out
    /// exactly as `write_texture` wants it, so the caller uploads one level per call.
    pub fn mip_chain(&self) -> Vec<Vec<u8>> {
        let layers = self.sprites.len().max(1);
        let mut size = self.size as usize;
        // Only alpha-tested sprites get their coverage held: a solid one has none to lose, and on a
        // translucent one alpha is real transparency, so stretching it would distort glass and water.
        let targets: Vec<Option<f32>> = (0..layers)
            .map(|layer| {
                let sprite = self.sprites.get(layer)?;
                (sprite.opacity == Opacity::Cutout)
                    .then(|| coverage(&self.pixels[layer * size * size * 4..], size * size, 1.0))
            })
            .collect();

        let mut levels = vec![self.pixels.clone()];
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
                if let Some(target) = targets[layer] {
                    match_coverage(dst, half * half, target);
                }
            }
            levels.push(level);
            size = half;
        }
        levels
    }
}

/// sRGB byte to the light it stands for. The atlas is `Rgba8UnormSrgb`, so its bytes are an
/// encoding and not a quantity: averaging them directly lands below the encoding of the average
/// light, and the shortfall compounds at every level of the chain.
static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|byte| {
        let c = byte as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
});

fn linear_to_srgb(light: f32) -> u8 {
    let c = if light <= 0.0031308 {
        light * 12.92
    } else {
        1.055 * light.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// The share of texels that would survive the fragment shader's `alpha < 0.5` test with every alpha
/// multiplied by `scale`.
fn coverage(pixels: &[u8], texels: usize, scale: f32) -> f32 {
    let kept = (0..texels)
        .filter(|i| pixels[i * 4 + 3] as f32 * scale >= 128.0)
        .count();
    kept as f32 / texels as f32
}

/// Scales a level's alpha so that its share of texels above the shader's cutoff matches mip 0's.
///
/// Averaging alpha lowers that share at every level, which is why alpha-tested foliage goes bald in
/// the distance instead of thinning. Coverage rises with the multiplier, so bisection finds it; ten
/// steps land well inside the granularity of any level worth correcting, and the chain is built
/// once at load.
fn match_coverage(pixels: &mut [u8], texels: usize, target: f32) {
    let (mut lo, mut hi) = (0.0f32, 4.0f32);
    for _ in 0..10 {
        let mid = 0.5 * (lo + hi);
        if coverage(pixels, texels, mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    for i in 0..texels {
        let alpha = &mut pixels[i * 4 + 3];
        *alpha = (*alpha as f32 * hi).round().min(255.0) as u8;
    }
}

/// Alpha-weighted so a cutout sprite does not bleed the colour of its fully transparent texels —
/// those carry arbitrary RGB in the vanilla pack and a plain average turns leaf edges black.
fn downsample_2x2(src: &[u8], stride: usize, x: usize, y: usize, dst: &mut [u8]) {
    let mut light = [0f32; 3];
    let mut alpha = 0u32;
    let mut weight = 0f32;
    for dy in 0..2 {
        for dx in 0..2 {
            let s = ((y + dy) * stride + x + dx) * 4;
            let a = src[s + 3] as u32;
            for c in 0..3 {
                light[c] += SRGB_TO_LINEAR[src[s + c] as usize] * a as f32;
            }
            alpha += a;
            weight += a as f32;
        }
    }
    if weight == 0.0 {
        dst[..4].copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    for c in 0..3 {
        dst[c] = linear_to_srgb(light[c] / weight);
    }
    // Alpha is a fraction already, not an encoded quantity, so it averages where colour cannot.
    dst[3] = (alpha / 4) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_sprite(size: usize, opacity: Opacity, pixels: Vec<u8>) -> SpriteArray {
        SpriteArray {
            size: size as u32,
            sprites: vec![Sprite { opacity }],
            pixels,
        }
    }

    /// White texels, opaque wherever `keep` says so.
    fn stencil(size: usize, keep: impl Fn(usize, usize) -> u8) -> Vec<u8> {
        let mut pixels = vec![255u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                pixels[(y * size + x) * 4 + 3] = keep(x, y);
            }
        }
        pixels
    }

    fn level_coverage(level: &[u8]) -> f32 {
        coverage(level, level.len() / 4, 1.0)
    }

    /// Averaging sRGB bytes averages an encoding rather than the light it stands for. Half the light
    /// of white encodes near 188; the byte average would claim 127 and darken every mip.
    #[test]
    fn downsampling_averages_light_rather_than_encoded_bytes() {
        let src = [
            0, 0, 0, 255, 255, 255, 255, 255, //
            0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut dst = [0u8; 4];
        downsample_2x2(&src, 2, 0, 0, &mut dst);
        assert_eq!(&dst[..3], &[188, 188, 188]);
    }

    /// A fine alpha-tested pattern averages to just under the shader's cutoff, so an uncorrected
    /// chain discards every texel of it: distant leaves go bald instead of thinning.
    #[test]
    fn an_alpha_tested_sprite_keeps_its_coverage_in_every_mip() {
        let size = 16;
        let pixels = stencil(size, |x, y| if (x + y) % 2 == 0 { 255 } else { 0 });
        let levels = one_sprite(size, Opacity::Cutout, pixels).mip_chain();
        assert_eq!(levels.len(), 5);
        for (level, data) in levels.iter().enumerate() {
            let kept = level_coverage(data);
            assert!(kept >= 0.5, "level {level} kept only {kept} of the sprite");
        }
    }

    /// Alpha on a translucent sprite is real transparency rather than a mask, so the coverage fix
    /// must leave it alone: stretched to a mask, glass at half alpha would vanish outright.
    #[test]
    fn a_translucent_sprite_keeps_its_alpha_as_it_is() {
        let size = 16;
        let levels =
            one_sprite(size, Opacity::Translucent, stencil(size, |_, _| 100)).mip_chain();
        for (level, data) in levels.iter().enumerate() {
            let alpha: Vec<u8> = data.iter().skip(3).step_by(4).copied().collect();
            assert!(
                alpha.iter().all(|&a| a == 100),
                "level {level} rescaled a translucent alpha"
            );
        }
    }

    /// A pack mixing resolutions must not force one size on everything: the sprites of each size
    /// get their own array, and layer numbering restarts inside it.
    #[test]
    fn sprites_of_different_sizes_land_in_different_arrays() {
        let mut registry = SpriteRegistry::new();
        let small = registry.intern("minecraft:block/stone").unwrap();
        let large = registry.intern("minecraft:block/water_flow").unwrap();
        let also_small = registry.intern("minecraft:block/dirt").unwrap();
        assert_ne!(small.array, large.array, "16x16 and 32x32 share an array");
        assert_eq!(small.array, also_small.array);
        assert_eq!((small.layer, also_small.layer), (0, 1), "layers restart per array");
        assert_eq!(large.layer, 0);

        let sizes: Vec<u32> = registry.arrays().iter().map(|array| array.size).collect();
        assert_eq!(sizes, [16, 32]);
        let counts: Vec<usize> = registry
            .arrays()
            .iter()
            .map(|array| array.sprites.len())
            .collect();
        assert_eq!(counts, [2, 1]);
        // Every layer of an array is its own size, so nothing was stretched to match a neighbour.
        for array in registry.arrays() {
            let expected = (array.size * array.size * 4) as usize * array.sprites.len();
            assert_eq!(array.pixels.len(), expected);
        }
    }

    /// Interning the same sprite twice must hand back the same place rather than a second copy.
    #[test]
    fn a_sprite_is_interned_once() {
        let mut registry = SpriteRegistry::new();
        let first = registry.intern("minecraft:block/stone").unwrap();
        let again = registry.intern("minecraft:block/stone").unwrap();
        assert_eq!(first, again);
        assert_eq!(registry.len(), 1);
    }

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
