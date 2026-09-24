use crate::anim;
use crate::model::{self, Pack};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use mcrs_minecraft_mesh::block::Pass;
use mcrs_minecraft_mesh::pack::{MAX_SPRITE_ARRAYS, MAX_SPRITES};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

pub const MISSING_SPRITE: &str = "minecraft:missingno";

/// Metal binds at most this many layers in one texture array.
pub const ARRAY_LAYERS: usize = 2048;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Opacity {
    Solid,
    Cutout,
    Translucent,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Sprite {
    pub array: u8,
    pub layer: u16,
    pub animation: Option<u16>,
}

pub struct Animation {
    pub array: u8,
    pub first_layer: u16,
    pub count: u32,
    pub frametime: u32,
    pub interpolate: bool,
}

pub struct SpriteArray {
    pub size: u32,
    layers: Vec<Opacity>,
    pixels: Vec<u8>,
}

/// The base texture a permuted sprite copies and the palette swap it applies.
type Permutation = (String, PaletteMapping);

pub struct SpriteRegistry {
    arrays: Vec<SpriteArray>,
    index: HashMap<String, u16>,
    table: Vec<Sprite>,
    animations: Vec<Animation>,
    permutations: HashMap<String, Permutation>,
}

struct Source {
    data: Vec<u8>,
    width: u32,
    height: u32,
    animation: Option<anim::Animation>,
}

impl SpriteRegistry {
    pub fn new() -> Self {
        Self {
            arrays: Vec::new(),
            index: HashMap::new(),
            table: Vec::new(),
            animations: Vec::new(),
            permutations: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn arrays(&self) -> &[SpriteArray] {
        &self.arrays
    }

    pub fn animations(&self) -> &[Animation] {
        &self.animations
    }

    pub fn table(&self) -> &[Sprite] {
        &self.table
    }

    pub fn sprite(&self, id: u16) -> Sprite {
        self.table[id as usize]
    }

    pub fn opacity(&self, id: u16) -> Opacity {
        let sprite = self.sprite(id);
        self.arrays[sprite.array as usize].layers[sprite.layer as usize]
    }

    /// Registers the paletted permutations every `atlases/*.json` declares, so
    /// an id such as `minecraft:trims/items/chestplate_trim_quartz` interns
    /// although no such file exists.
    pub fn load_atlases(&mut self, pack: &Pack) -> Result<(), String> {
        for (id, bytes) in pack.entries("atlases", "json") {
            let sources: AtlasSources = serde_json::from_slice(bytes)
                .map_err(|error| format!("cannot parse atlas {id}: {error}"))?;
            self.permutations.extend(sources.permutations(pack)?);
        }
        Ok(())
    }

    fn source(&self, pack: &Pack, id: &str) -> Result<Source, String> {
        let path = model::resource_path(id, "textures", "png");
        if let Some(bytes) = pack.get(&path) {
            let (data, width, height) = decode_png(bytes, &path)?;
            return Ok(Source {
                data,
                width,
                height,
                animation: anim::read(pack, &path)?,
            });
        }
        if let Some((base, mapping)) = self.permutations.get(id) {
            let base_path = model::resource_path(base, "textures", "png");
            let (mut data, width, height) = decode_png(pack.read(&base_path)?, &base_path)?;
            for pixel in data.as_chunks_mut::<4>().0 {
                *pixel = rgba(mapping.apply(argb(pixel)));
            }
            return Ok(Source {
                data,
                width,
                height,
                animation: anim::read(pack, &base_path)?,
            });
        }
        if model::split_id(id) == model::split_id(MISSING_SPRITE) {
            return Ok(Source {
                data: missing_image(),
                width: 16,
                height: 16,
                animation: None,
            });
        }
        Err(format!("{path} is not in the resource pack"))
    }

    /// The pixels of every frame the sprite holds, level zero, row-major RGBA.
    pub fn frames(&self, id: u16) -> Vec<&[u8]> {
        let sprite = self.sprite(id);
        let array = &self.arrays[sprite.array as usize];
        let count = sprite
            .animation
            .map_or(1, |index| self.animations[index as usize].count as usize);
        (0..count)
            .map(|step| array.layer(sprite.layer as usize + step))
            .collect()
    }

    pub fn intern(&mut self, pack: &Pack, id: &str) -> Result<u16, String> {
        if let Some(&sprite) = self.index.get(id) {
            return Ok(sprite);
        }
        if self.table.len() >= MAX_SPRITES {
            return Err(format!(
                "{id} would be sprite {MAX_SPRITES}, past what a face can name"
            ));
        }
        let path = model::resource_path(id, "textures", "png");
        let Source {
            data,
            width,
            height,
            animation,
        } = self.source(pack, id)?;
        let image_size = (width, height);
        let (frame_width, frame_height) = match &animation {
            Some(animation) => animation.frame_size(image_size),
            None => (width, width),
        };
        if frame_width != frame_height {
            return Err(format!(
                "{path} has {frame_width}x{frame_height} frames, and every layer of an array is square",
            ));
        }
        let side = frame_width;

        let sequence = match &animation {
            Some(animation) => animation.unroll(id, image_size),
            None => anim::Unrolled {
                frames: Vec::new(),
                frametime: 1,
            },
        };
        let frames: &[u32] = if sequence.frames.is_empty() {
            &[0]
        } else {
            &sequence.frames
        };
        let mut pixels = Vec::with_capacity(frames.len() * (side * side * 4) as usize);
        for &frame in frames {
            let cut = cut(&data, image_size.0, side, frame)
                .ok_or_else(|| format!("{path} has no frame {frame}"))?;
            pixels.extend_from_slice(&cut);
        }
        let opacity = opacity_of(&pixels);

        let fits = |array: &SpriteArray| {
            array.size == side && array.layers.len() + frames.len() <= ARRAY_LAYERS
        };
        let index = match self.arrays.iter().position(fits) {
            Some(index) => index,
            None => {
                if self.arrays.len() >= MAX_SPRITE_ARRAYS {
                    return Err(format!(
                        "{id} needs a {}th texture array, but the shaders bind {MAX_SPRITE_ARRAYS}",
                        self.arrays.len() + 1
                    ));
                }
                self.arrays.push(SpriteArray {
                    size: side,
                    layers: Vec::new(),
                    pixels: Vec::new(),
                });
                self.arrays.len() - 1
            }
        };
        let array = &mut self.arrays[index];
        let layer = array.layers.len() as u16;
        array
            .layers
            .extend(std::iter::repeat_n(opacity, frames.len()));
        array.pixels.extend_from_slice(&pixels);
        let animation = (!sequence.frames.is_empty()).then(|| {
            self.animations.push(Animation {
                array: index as u8,
                first_layer: layer,
                count: frames.len() as u32,
                frametime: sequence.frametime,
                interpolate: animation.is_some_and(|a| a.interpolate),
            });
            (self.animations.len() - 1) as u16
        });
        let sprite = self.table.len() as u16;
        self.table.push(Sprite {
            array: index as u8,
            layer,
            animation,
        });
        self.index.insert(id.to_string(), sprite);
        Ok(sprite)
    }
}

pub fn decode_png(bytes: &[u8], path: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .map_err(|error| format!("cannot decode {path}: {error}"))?;
    let (width, height) = (image.width(), image.height());
    let data = image
        .data
        .ok_or_else(|| format!("{path} decoded without pixel data"))?;
    Ok((data, width, height))
}

fn argb(rgba: &[u8; 4]) -> u32 {
    u32::from_be_bytes([rgba[3], rgba[0], rgba[1], rgba[2]])
}

pub(crate) fn rgba(argb: u32) -> [u8; 4] {
    let [a, r, g, b] = argb.to_be_bytes();
    [r, g, b, a]
}

/// The 16x16 magenta and black checker vanilla generates for a texture it cannot find.
fn missing_image() -> Vec<u8> {
    (0..256)
        .flat_map(|i| {
            if (i % 16 < 8) == (i / 16 < 8) {
                [0xF8, 0x00, 0xF8, 0xFF]
            } else {
                [0x00, 0x00, 0x00, 0xFF]
            }
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AtlasSources {
    sources: Vec<AtlasSource>,
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum AtlasSource {
    #[serde(
        rename = "minecraft:paletted_permutations",
        alias = "paletted_permutations"
    )]
    PalettedPermutations {
        textures: Vec<String>,
        palette_key: String,
        permutations: BTreeMap<String, String>,
        #[serde(default = "underscore")]
        separator: String,
    },
    #[serde(other)]
    Other,
}

fn underscore() -> String {
    "_".to_string()
}

impl AtlasSources {
    /// Every permuted sprite id with the base texture it copies and the palette swap it applies.
    /// Only paletted permutations create sprites here; the other sources name files that
    /// intern on demand.
    fn permutations(&self, pack: &Pack) -> Result<Vec<(String, Permutation)>, String> {
        let mut out = Vec::new();
        for source in &self.sources {
            let AtlasSource::PalettedPermutations {
                textures,
                palette_key,
                permutations,
                separator,
            } = source
            else {
                continue;
            };
            let base = Palette::load(pack, palette_key)?;
            for (suffix, palette) in permutations {
                let mapping = PaletteMapping::create(&base, &Palette::load(pack, palette)?)?;
                for texture in textures {
                    out.push((
                        format!("{texture}{separator}{suffix}"),
                        (texture.clone(), mapping.clone()),
                    ));
                }
            }
        }
        Ok(out)
    }
}

struct Palette(Vec<u32>);

impl Palette {
    fn load(pack: &Pack, id: &str) -> Result<Self, String> {
        let path = model::resource_path(id, "textures/palettes", "png");
        let (data, _, _) = decode_png(pack.read(&path)?, &path)?;
        Ok(Self(data.as_chunks::<4>().0.iter().map(argb).collect()))
    }
}

#[derive(Clone)]
pub struct PaletteMapping(HashMap<u32, u32>);

impl PaletteMapping {
    fn create(base: &Palette, target: &Palette) -> Result<Self, String> {
        if base.0.len() != target.0.len() {
            return Err(format!(
                "PaletteMapping has different sizes: {} != {}",
                base.0.len(),
                target.0.len()
            ));
        }
        Ok(Self(
            base.0
                .iter()
                .zip(&target.0)
                .filter(|(key, _)| **key >> 24 != 0)
                .map(|(key, value)| (key | 0xFF00_0000, *value))
                .collect(),
        ))
    }

    fn apply(&self, pixel: u32) -> u32 {
        let alpha = pixel >> 24;
        if alpha == 0 {
            return pixel;
        }
        let opaque = pixel | 0xFF00_0000;
        let target = *self.0.get(&opaque).unwrap_or(&opaque);
        let target_alpha = target >> 24;
        (alpha * target_alpha / 255) << 24 | (target & 0x00FF_FFFF)
    }
}

fn cut(data: &[u8], image_width: u32, side: u32, frame: u32) -> Option<Vec<u8>> {
    let columns = (image_width / side).max(1);
    let (left, top) = (frame % columns * side, frame / columns * side);
    let mut pixels = Vec::with_capacity((side * side * 4) as usize);
    for row in 0..side {
        let start = (((top + row) * image_width + left) * 4) as usize;
        pixels.extend_from_slice(data.get(start..start + (side * 4) as usize)?);
    }
    Some(pixels)
}

fn opacity_of(pixels: &[u8]) -> Opacity {
    let mut opaque = true;
    let mut binary = true;
    for i in (3..pixels.len()).step_by(4) {
        let alpha = pixels[i];
        if alpha != 255 {
            opaque = false;
        }
        if alpha != 255 && alpha != 0 {
            binary = false;
        }
    }
    match (opaque, binary) {
        (true, _) => Opacity::Solid,
        (false, true) => Opacity::Cutout,
        (false, false) => Opacity::Translucent,
    }
}

impl SpriteArray {
    pub fn layers(&self) -> u32 {
        self.layers.len() as u32
    }

    fn layer(&self, layer: usize) -> &[u8] {
        let stride = (self.size * self.size * 4) as usize;
        &self.pixels[layer * stride..(layer + 1) * stride]
    }

    /// The mip chain of the layers from `from` on, in the layout the whole chain uses.
    pub fn mips(&self, from: usize) -> Vec<Vec<u8>> {
        let stride = (self.size * self.size * 4) as usize;
        mip_levels(
            &self.pixels[from * stride..],
            &self.layers[from..],
            self.size as usize,
        )
    }
}

/// Every level of every layer, level zero being the pixels as given. A cutout layer keeps the
/// share of texels its alpha test passes at every level, so a leaf block does not thin out with
/// distance.
fn mip_levels(pixels: &[u8], opacities: &[Opacity], mut size: usize) -> Vec<Vec<u8>> {
    let layers = opacities.len();
    let targets: Vec<Option<f32>> = (0..layers)
        .map(|layer| {
            (opacities[layer] == Opacity::Cutout)
                .then(|| coverage(&pixels[layer * size * size * 4..], size * size, 1.0))
        })
        .collect();

    let mut levels = vec![pixels[..layers * size * size * 4].to_vec()];
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

static SRGB_TO_LINEAR: LazyLock<[f32; 256]> =
    LazyLock::new(|| std::array::from_fn(|byte| Srgba::gamma_function(byte as f32 / 255.0)));

fn linear_to_srgb(light: f32) -> u8 {
    (Srgba::gamma_function_inverse(light) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn coverage(pixels: &[u8], texels: usize, scale: f32) -> f32 {
    let kept = (0..texels)
        .filter(|i| pixels[i * 4 + 3] as f32 * scale >= 128.0)
        .count();
    kept as f32 / texels as f32
}

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
    dst[3] = (alpha / 4) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_sprite(size: usize, opacity: Opacity, pixels: Vec<u8>) -> SpriteArray {
        SpriteArray {
            size: size as u32,
            layers: vec![opacity],
            pixels,
        }
    }

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

    #[test]
    fn downsampling_averages_light_rather_than_encoded_bytes() {
        let src = [0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut dst = [0u8; 4];
        downsample_2x2(&src, 2, 0, 0, &mut dst);
        assert_eq!(&dst[..3], &[188, 188, 188]);
    }

    #[test]
    fn an_alpha_tested_sprite_keeps_its_coverage_in_every_mip() {
        let size = 16;
        let pixels = stencil(size, |x, y| if (x + y) % 2 == 0 { 255 } else { 0 });
        let levels = one_sprite(size, Opacity::Cutout, pixels).mips(0);
        assert_eq!(levels.len(), 5);
        for (level, data) in levels.iter().enumerate() {
            let kept = level_coverage(data);
            assert!(kept >= 0.5, "level {level} kept only {kept} of the sprite");
        }
    }

    #[test]
    fn a_translucent_sprite_keeps_its_alpha_as_it_is() {
        let size = 16;
        let levels = one_sprite(size, Opacity::Translucent, stencil(size, |_, _| 100)).mips(0);
        for (level, data) in levels.iter().enumerate() {
            let alpha: Vec<u8> = data.iter().skip(3).step_by(4).copied().collect();
            assert!(
                alpha.iter().all(|&a| a == 100),
                "level {level} rescaled a translucent alpha"
            );
        }
    }

    #[test]
    fn the_mips_of_a_tail_of_stills_match_the_whole_chain() {
        let mut registry = SpriteRegistry::new();
        for id in [
            "minecraft:block/stone",
            "minecraft:block/dirt",
            "minecraft:block/oak_leaves",
        ] {
            registry.intern(Pack::corpus(), id).unwrap();
        }
        let array = &registry.arrays()[0];
        let whole = array.mips(0);
        let tail = array.mips(1);
        assert_eq!(tail.len(), whole.len());
        for (level, (all, some)) in whole.iter().zip(&tail).enumerate() {
            let size = (array.size as usize >> level).max(1);
            let stride = size * size * 4;
            assert_eq!(&all[stride..], &some[..], "level {level}");
        }
    }

    #[test]
    fn sprites_of_different_sizes_land_in_different_arrays() {
        let mut registry = SpriteRegistry::new();
        let small = registry
            .intern(Pack::corpus(), "minecraft:block/stone")
            .unwrap();
        let large = registry
            .intern(Pack::corpus(), "minecraft:block/water_flow")
            .unwrap();
        let also_small = registry
            .intern(Pack::corpus(), "minecraft:block/dirt")
            .unwrap();
        assert_eq!(
            (small, large, also_small),
            (0, 1, 2),
            "ids count up as interned"
        );
        let (small, large, also_small) = (
            registry.sprite(small),
            registry.sprite(large),
            registry.sprite(also_small),
        );
        assert_ne!(small.array, large.array, "16x16 and 32x32 share an array");
        assert_eq!(small.array, also_small.array);
        assert_eq!(
            (small.layer, also_small.layer),
            (0, 1),
            "layers restart per array"
        );
        assert_eq!(large.layer, 0);
        assert_eq!(large.animation, Some(0));

        let sizes: Vec<u32> = registry.arrays().iter().map(|array| array.size).collect();
        assert_eq!(sizes, [16, 32]);
        let counts: Vec<usize> = (0..registry.arrays().len())
            .map(|array| {
                registry
                    .table()
                    .iter()
                    .filter(|s| s.array as usize == array)
                    .count()
            })
            .collect();
        assert_eq!(counts, [2, 1]);
        for array in registry.arrays() {
            let expected = (array.size * array.size * 4) as usize * array.layers() as usize;
            assert_eq!(array.pixels.len(), expected);
        }
    }

    #[test]
    fn every_step_of_an_animation_gets_its_own_layer() {
        let mut registry = SpriteRegistry::new();
        registry
            .intern(Pack::corpus(), "minecraft:block/stone")
            .unwrap();
        registry
            .intern(Pack::corpus(), "minecraft:block/kelp")
            .unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(registry.len(), 2);
        assert_eq!(registry.animations().len(), 1);
        assert_eq!(array.layers(), 21);
        let texels = (array.size * array.size * 4) as usize;
        assert_eq!(array.pixels.len(), texels * 21);
    }

    fn layer(array: &SpriteArray, layer: usize) -> &[u8] {
        array.layer(layer)
    }

    #[test]
    fn a_sequence_that_revisits_a_frame_lays_it_down_twice() {
        let mut registry = SpriteRegistry::new();
        registry
            .intern(Pack::corpus(), "minecraft:block/lava_still")
            .unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(array.layers(), 38);
        assert_eq!(layer(array, 18), layer(array, 20));
        assert_ne!(layer(array, 18), layer(array, 19));
    }

    #[test]
    fn a_sequence_out_of_order_is_laid_out_in_the_order_it_names() {
        let mut registry = SpriteRegistry::new();
        registry
            .intern(Pack::corpus(), "minecraft:block/prismarine")
            .unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(array.layers(), 22);
        assert_eq!(layer(array, 0), layer(array, 2));
        assert_ne!(layer(array, 0), layer(array, 1));
    }

    #[test]
    fn a_sequence_starting_part_way_through_the_image_starts_there() {
        let mut registry = SpriteRegistry::new();
        registry
            .intern(Pack::corpus(), "minecraft:block/fire_0")
            .unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(array.layers(), 32);
        let stride = (array.size * array.size * 4) as usize;
        let image = Pack::corpus()
            .read(&model::resource_path(
                "minecraft:block/fire_0",
                "textures",
                "png",
            ))
            .unwrap();
        let image = Image::from_buffer(
            image,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .unwrap();
        let data = image.data.unwrap();
        assert_eq!(layer(array, 0), &data[16 * stride..17 * stride]);
        assert_eq!(layer(array, 16), &data[..stride]);
    }

    fn source_frames(id: &str) -> Vec<Vec<u8>> {
        let bytes = Pack::corpus()
            .read(&model::resource_path(id, "textures", "png"))
            .unwrap();
        let image = Image::from_buffer(
            bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .unwrap();
        let side = image.width() as usize;
        let stride = side * side * 4;
        image
            .data
            .unwrap()
            .chunks_exact(stride)
            .map(<[u8]>::to_vec)
            .collect()
    }

    #[test]
    fn an_animation_names_its_own_layers_whatever_order_the_interning_took() {
        let mut registry = SpriteRegistry::new();
        let stone = registry
            .intern(Pack::corpus(), "minecraft:block/stone")
            .unwrap();
        let kelp = registry
            .intern(Pack::corpus(), "minecraft:block/kelp")
            .unwrap();
        let dirt = registry
            .intern(Pack::corpus(), "minecraft:block/dirt")
            .unwrap();
        let seagrass = registry
            .intern(Pack::corpus(), "minecraft:block/seagrass")
            .unwrap();

        let array = &registry.arrays()[0];
        let layer = |index: usize| array.layer(index);

        assert_eq!(
            (registry.sprite(stone).layer, registry.sprite(dirt).layer),
            (0, 21),
            "a still takes the layer after whatever came before it"
        );
        assert_eq!(layer(0), &source_frames("minecraft:block/stone")[0][..]);
        assert_eq!(layer(21), &source_frames("minecraft:block/dirt")[0][..]);

        for (id, sprite) in [
            ("minecraft:block/kelp", kelp),
            ("minecraft:block/seagrass", seagrass),
        ] {
            let sprite = registry.sprite(sprite);
            let animation =
                &registry.animations()[sprite.animation.expect("the sprite animates") as usize];
            let base = animation.first_layer as usize;
            assert_eq!(base, sprite.layer as usize);
            let frames = source_frames(id);
            assert_eq!(animation.count as usize, frames.len());
            for step in 0..animation.count as usize {
                assert_eq!(layer(base + step), &frames[step][..], "{id} step {step}");
            }
        }
    }

    #[test]
    fn a_frame_is_cut_out_of_the_grid_it_sits_in() {
        let mut image = vec![0u8; 4 * 4 * 4];
        for y in 0..4usize {
            for x in 0..4usize {
                image[(y * 4 + x) * 4] = (y / 2 * 2 + x / 2) as u8;
            }
        }
        for frame in 0..4u32 {
            let cut = cut(&image, 4, 2, frame).expect("the frame is inside the image");
            assert!(
                cut.iter().step_by(4).all(|&byte| byte as u32 == frame),
                "frame {frame} was cut from the wrong cell",
            );
        }
        assert!(
            cut(&image, 4, 2, 4).is_none(),
            "a frame past the last one is not there"
        );
    }

    #[test]
    fn opacity_is_taken_over_every_frame_rather_than_the_first() {
        let opaque = [255u8, 255, 255, 255];
        let half = [255u8, 255, 255, 128];
        let cut_out = [255u8, 255, 255, 0];
        assert_eq!(opacity_of(&opaque), Opacity::Solid);
        assert_eq!(opacity_of(&[opaque, opaque].concat()), Opacity::Solid);
        assert_eq!(opacity_of(&[opaque, cut_out].concat()), Opacity::Cutout);
        assert_eq!(opacity_of(&[opaque, half].concat()), Opacity::Translucent);
        assert_eq!(opacity_of(&[cut_out, half].concat()), Opacity::Translucent);
    }

    #[test]
    fn a_sprite_is_interned_once() {
        let mut registry = SpriteRegistry::new();
        let first = registry
            .intern(Pack::corpus(), "minecraft:block/stone")
            .unwrap();
        let again = registry
            .intern(Pack::corpus(), "minecraft:block/stone")
            .unwrap();
        assert_eq!(first, again);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn downsampling_ignores_the_colour_of_transparent_texels() {
        let src = [255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut dst = [0u8; 4];
        downsample_2x2(&src, 2, 0, 0, &mut dst);
        assert_eq!(&dst[..3], &[255, 255, 255]);
        assert_eq!(dst[3], 63);
    }

    #[test]
    fn every_atlas_parses_and_the_item_trims_permute_into_sprites() {
        let pack = Pack::corpus();
        let mut permuted = 0;
        for (id, bytes) in pack.entries("atlases", "json") {
            let sources: AtlasSources =
                serde_json::from_slice(bytes).unwrap_or_else(|e| panic!("{id}: {e}"));
            permuted += sources.permutations(pack).unwrap().len();
        }
        assert_eq!(permuted, 64);
        let mut registry = SpriteRegistry::new();
        registry.load_atlases(pack).unwrap();
        let quartz = registry
            .intern(pack, "minecraft:trims/items/chestplate_trim_quartz")
            .unwrap();
        let base = registry
            .intern(pack, "minecraft:trims/items/chestplate_trim")
            .unwrap();
        let quartz_pixels = registry.frames(quartz)[0].to_vec();
        let base_pixels = registry.frames(base)[0].to_vec();
        assert_ne!(quartz_pixels, base_pixels);
        for (q, b) in quartz_pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(base_pixels.as_chunks::<4>().0)
        {
            assert_eq!(q[3] == 0, b[3] == 0);
        }
        assert!(
            registry
                .intern(pack, "minecraft:trims/items/chestplate_trim_nope")
                .is_err()
        );
    }

    #[test]
    fn the_missing_sprite_is_a_generated_checker() {
        let mut registry = SpriteRegistry::new();
        let sprite = registry.intern(Pack::corpus(), MISSING_SPRITE).unwrap();
        let pixels = registry.frames(sprite)[0];
        assert_eq!(pixels.len(), 16 * 16 * 4);
        assert_eq!(&pixels[..4], &[0xF8, 0, 0xF8, 0xFF]);
        assert_eq!(&pixels[8 * 4..8 * 4 + 4], &[0, 0, 0, 0xFF]);
        assert_eq!(registry.opacity(sprite), Opacity::Solid);
    }

    #[test]
    fn a_palette_mapping_scales_alpha_and_leaves_unknown_colours_alone() {
        let mapping = PaletteMapping::create(
            &Palette(vec![0xFF11_2233, 0x0000_0000]),
            &Palette(vec![0x8044_5566, 0xFF00_0000]),
        )
        .unwrap();
        assert_eq!(mapping.apply(0xFF11_2233), 0x8044_5566);
        assert_eq!(mapping.apply(0x8011_2233), 0x4044_5566);
        assert_eq!(mapping.apply(0x00AA_BBCC), 0x00AA_BBCC);
        assert_eq!(mapping.apply(0xFFAA_BBCC), 0xFFAA_BBCC);
        assert!(PaletteMapping::create(&Palette(vec![1]), &Palette(vec![1, 2])).is_err());
    }
}

impl From<Opacity> for Pass {
    fn from(opacity: Opacity) -> Pass {
        match opacity {
            Opacity::Solid => Pass::Solid,
            Opacity::Cutout => Pass::Cutout,
            Opacity::Translucent => Pass::Translucent,
        }
    }
}
