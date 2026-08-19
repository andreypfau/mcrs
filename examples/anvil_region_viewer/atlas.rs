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

use crate::anim;
use crate::model;
use crate::pack::MAX_SPRITES;

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

/// Where a sprite lives: which array, and which layer of that array.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct SpriteRef {
    pub array: u8,
    pub layer: u16,
}

/// One animated sprite: a run of layers, one per step of its sequence, and how fast to walk it.
///
/// There is no schedule to go with it. The sequence was laid out one step to a layer, so the step
/// showing now is arithmetic on the clock and nothing has to be looked up per fragment.
pub struct Animation {
    pub array: u8,
    /// Where the run starts among its array's animation layers, which follow all of its stills.
    frame_base: u32,
    pub count: u32,
    /// Ticks one step lasts.
    pub frametime: u32,
    pub interpolate: bool,
    opacity: Opacity,
}

/// Every sprite of one resolution, which becomes one array texture. Layers are square: the still
/// sprites come first, one layer each, and the steps of every animation follow them.
pub struct SpriteArray {
    /// Side length of every layer.
    pub size: u32,
    /// What each still sprite's alpha channel makes of it. A still sprite's layer is its place here.
    stills: Vec<Opacity>,
    /// The same for each animation layer, in the order the animations were interned.
    frames: Vec<Opacity>,
    /// How many of this array's sprites animate.
    animated: usize,
    /// RGBA8 pixels of the still layers, layer-major, mip 0 only. Mips are derived at upload time.
    still_pixels: Vec<u8>,
    /// The same for the animation layers, which are uploaded after the stills.
    frame_pixels: Vec<u8>,
}

pub struct SpriteRegistry {
    arrays: Vec<SpriteArray>,
    index: HashMap<String, SpriteRef>,
    /// Every animation of every resolution. A quad names one of these by counting down from the top
    /// of its layer field, so a still sprite is anything below that and costs no lookup at all.
    animations: Vec<Animation>,
}

impl SpriteRegistry {
    pub fn new() -> Self {
        Self {
            arrays: Vec::new(),
            index: HashMap::new(),
            animations: Vec::new(),
        }
    }

    /// Every sprite of every resolution.
    pub fn len(&self) -> usize {
        self.arrays.iter().map(SpriteArray::sprites).sum()
    }

    pub fn arrays(&self) -> &[SpriteArray] {
        &self.arrays
    }

    pub fn animations(&self) -> &[Animation] {
        &self.animations
    }

    /// The lowest layer number that names an animation instead of a layer of the array. Animations
    /// take the top of the field so that telling the two apart is one comparison against a uniform.
    pub fn animated_from(&self) -> u32 {
        (MAX_SPRITES - self.animations.len()) as u32
    }

    /// Where an animation's run of layers starts in its array, once the still sprites that come
    /// before it are counted.
    pub fn base_layer(&self, animation: &Animation) -> u32 {
        self.arrays[animation.array as usize].stills.len() as u32 + animation.frame_base
    }

    fn animation(&self, layer: u16) -> Option<&Animation> {
        let index = (MAX_SPRITES - 1).checked_sub(layer as usize)?;
        self.animations.get(index)
    }

    pub fn opacity(&self, sprite: SpriteRef) -> Opacity {
        match self.animation(sprite.layer) {
            Some(animation) => animation.opacity,
            None => self.arrays[sprite.array as usize].stills[sprite.layer as usize],
        }
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
        let animation = anim::read(&path)?;
        let image_size = (image.width(), image.height());
        let (frame_width, frame_height) = match &animation {
            Some(animation) => animation.frame_size(image_size),
            None => (image.width(), image.width()),
        };
        if frame_width != frame_height {
            return Err(format!(
                "{} has {frame_width}x{frame_height} frames, and every layer of an array is square",
                path.display(),
            ));
        }
        let side = frame_width;
        let data = image
            .data
            .ok_or_else(|| format!("{} decoded without pixel data", path.display()))?;

        // Every step of the sequence is resident as its own layer, so a still sprite is the case
        // where there is no sequence rather than a different kind of thing.
        let sequence = match &animation {
            Some(animation) => animation.unroll(id, image_size),
            None => anim::Unrolled { frames: Vec::new(), frametime: 1 },
        };
        let frames: &[u32] = if sequence.frames.is_empty() { &[0] } else { &sequence.frames };
        let mut pixels = Vec::with_capacity(frames.len() * (side * side * 4) as usize);
        for &frame in frames {
            let cut = cut(&data, image_size.0, side, frame).ok_or_else(|| {
                format!("{} has no frame {frame}", path.display())
            })?;
            pixels.extend_from_slice(&cut);
        }
        // Taken over every frame at once: were a sprite's first frame opaque and another one not,
        // the two would want different passes, and a sprite is drawn in exactly one.
        let opacity = opacity_of(&pixels);

        let index = match self.arrays.iter().position(|array| array.size == side) {
            Some(index) => index,
            None => {
                self.arrays.push(SpriteArray {
                    size: side,
                    stills: Vec::new(),
                    frames: Vec::new(),
                    animated: 0,
                    still_pixels: Vec::new(),
                    frame_pixels: Vec::new(),
                });
                self.arrays.len() - 1
            }
        };
        let array = &mut self.arrays[index];
        let sprite = if sequence.frames.is_empty() {
            let sprite = SpriteRef {
                array: index as u8,
                layer: array.stills.len() as u16,
            };
            array.stills.push(opacity);
            array.still_pixels.extend_from_slice(&pixels);
            sprite
        } else {
            let animation = Animation {
                array: index as u8,
                frame_base: array.frames.len() as u32,
                count: frames.len() as u32,
                frametime: sequence.frametime,
                interpolate: animation.is_some_and(|a| a.interpolate),
                opacity,
            };
            array.animated += 1;
            array.frames.extend(std::iter::repeat_n(opacity, frames.len()));
            array.frame_pixels.extend_from_slice(&pixels);
            // Counting down from the top of the layer field rather than up from zero: what a layer
            // number below the count of animations means then does not depend on how many sprites
            // the pack turned out to have.
            let sprite = SpriteRef {
                array: index as u8,
                layer: (MAX_SPRITES - 1 - self.animations.len()) as u16,
            };
            self.animations.push(animation);
            sprite
        };
        self.index.insert(id.to_string(), sprite);
        Ok(sprite)
    }
}

/// One frame of a sprite's image, which sits in it as a cell of a grid running across the image
/// and then down it.
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
        (self.stills.len() + self.frames.len()).max(1) as u32
    }

    /// Sprite ids interned into this array, as against the layers they occupy.
    pub fn sprites(&self) -> usize {
        self.stills.len() + self.animated
    }

    /// How many of this array's sprites animate.
    pub fn animated(&self) -> usize {
        self.animated
    }

    /// Layer numbers a still sprite of this array reaches, which is what the animations at the top
    /// of the field have to stay clear of.
    pub fn stills(&self) -> usize {
        self.stills.len()
    }

    /// Mip 0 followed by every smaller level, layer-major within each level. Each level is laid out
    /// exactly as `write_texture` wants it, so the caller uploads one level per call.
    pub fn mip_chain(&self) -> Vec<Vec<u8>> {
        let opacities: Vec<Opacity> = self.stills.iter().chain(&self.frames).copied().collect();
        let pixels = [&self.still_pixels[..], &self.frame_pixels[..]].concat();
        let layers = opacities.len().max(1);
        let mut size = self.size as usize;
        // Only alpha-tested sprites get their coverage held: a solid one has none to lose, and on a
        // translucent one alpha is real transparency, so stretching it would distort glass and water.
        let targets: Vec<Option<f32>> = (0..layers)
            .map(|layer| {
                (*opacities.get(layer)? == Opacity::Cutout)
                    .then(|| coverage(&pixels[layer * size * size * 4..], size * size, 1.0))
            })
            .collect();

        let mut levels = vec![pixels];
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
            stills: vec![opacity],
            frames: Vec::new(),
            animated: 0,
            still_pixels: pixels,
            frame_pixels: Vec::new(),
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
        // water_flow animates, so its layer number names an animation from the top of the field
        // rather than a layer of its array.
        assert_eq!(large.layer as u32, registry.animated_from());

        let sizes: Vec<u32> = registry.arrays().iter().map(|array| array.size).collect();
        assert_eq!(sizes, [16, 32]);
        let counts: Vec<usize> = registry
            .arrays()
            .iter()
            .map(SpriteArray::sprites)
            .collect();
        assert_eq!(counts, [2, 1]);
        // Every layer of an array is its own size, so nothing was stretched to match a neighbour.
        for array in registry.arrays() {
            let expected = (array.size * array.size * 4) as usize * array.layers() as usize;
            assert_eq!(array.still_pixels.len() + array.frame_pixels.len(), expected);
        }
    }

    /// An animated sprite keeps every step of its sequence resident, one to a layer, so the array
    /// grows by the length of the run rather than by one.
    #[test]
    fn every_step_of_an_animation_gets_its_own_layer() {
        let mut registry = SpriteRegistry::new();
        registry.intern("minecraft:block/stone").unwrap();
        // Twenty frames in order, which is the shape fifty of the pack's animations have.
        registry.intern("minecraft:block/kelp").unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(array.sprites(), 2);
        assert_eq!(array.animated(), 1);
        assert_eq!(array.layers(), 21);
        let texels = (array.size * array.size * 4) as usize;
        assert_eq!(array.still_pixels.len(), texels);
        assert_eq!(array.frame_pixels.len(), texels * 20);
    }

    /// Pixels of one animation layer, which follow the array's still sprites.
    fn layer(array: &SpriteArray, layer: usize) -> &[u8] {
        let stride = (array.size * array.size * 4) as usize;
        &array.frame_pixels[layer * stride..(layer + 1) * stride]
    }

    /// A sequence that revisits a frame gets that frame again as another layer rather than a
    /// schedule pointing back at the first one. That is what leaves an animation describable as a
    /// first layer, a length and one duration.
    #[test]
    fn a_sequence_that_revisits_a_frame_lays_it_down_twice() {
        let mut registry = SpriteRegistry::new();
        // Twenty frames run forwards and then back, which is thirty-eight steps.
        registry.intern("minecraft:block/lava_still").unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(array.layers(), 38);
        // The run turns around on the last frame, so the step after the turn repeats the one before
        // it, and both are resident.
        assert_eq!(layer(array, 18), layer(array, 20));
        assert_ne!(layer(array, 18), layer(array, 19));
    }

    /// A sequence in an arbitrary order with repeats is laid out in that order, one layer per step.
    #[test]
    fn a_sequence_out_of_order_is_laid_out_in_the_order_it_names() {
        let mut registry = SpriteRegistry::new();
        // Four frames named twenty-two times over.
        registry.intern("minecraft:block/prismarine").unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(array.layers(), 22);
        // The listed order is 0, 1, 0, 2, ... so the first and third steps are the same frame.
        assert_eq!(layer(array, 0), layer(array, 2));
        assert_ne!(layer(array, 0), layer(array, 1));
    }

    /// A sequence may start anywhere in the image; the frames the image happens to hold first stop
    /// being where the animation starts.
    #[test]
    fn a_sequence_starting_part_way_through_the_image_starts_there() {
        let mut registry = SpriteRegistry::new();
        // Thirty-two frames named starting at the seventeenth.
        registry.intern("minecraft:block/fire_0").unwrap();
        let array = &registry.arrays()[0];
        assert_eq!(array.layers(), 32);
        let stride = (array.size * array.size * 4) as usize;
        let image = std::fs::read(crate::model::resource_path(
            "minecraft:block/fire_0",
            "textures",
            "png",
        ))
        .unwrap();
        let image = Image::from_buffer(
            &image,
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

    /// Every layer of a sprite's own image, decoded straight from the file.
    fn source_frames(id: &str) -> Vec<Vec<u8>> {
        let bytes = std::fs::read(model::resource_path(id, "textures", "png")).unwrap();
        let image = Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .unwrap();
        let side = image.width() as usize;
        let stride = side * side * 4;
        image.data.unwrap().chunks_exact(stride).map(<[u8]>::to_vec).collect()
    }

    /// Still sprites and animations arrive interleaved as the region interns them, but they do not
    /// lie interleaved: an array's stills come first and the animation runs follow, so where a run
    /// starts is only settled once the last still is in. A run pointing one sprite off shows the
    /// wrong texture entirely and nothing anywhere would report it.
    #[test]
    fn an_animation_names_its_own_layers_whatever_order_the_interning_took() {
        let mut registry = SpriteRegistry::new();
        let stone = registry.intern("minecraft:block/stone").unwrap();
        let kelp = registry.intern("minecraft:block/kelp").unwrap();
        let dirt = registry.intern("minecraft:block/dirt").unwrap();
        let seagrass = registry.intern("minecraft:block/seagrass").unwrap();

        let array = &registry.arrays()[0];
        let stride = (array.size * array.size * 4) as usize;
        let resident = [&array.still_pixels[..], &array.frame_pixels[..]].concat();
        let layer = |index: usize| &resident[index * stride..(index + 1) * stride];

        assert_eq!((stone.layer, dirt.layer), (0, 1), "stills keep the low layers");
        assert_eq!(layer(0), &source_frames("minecraft:block/stone")[0][..]);
        assert_eq!(layer(1), &source_frames("minecraft:block/dirt")[0][..]);

        for (id, sprite) in [
            ("minecraft:block/kelp", kelp),
            ("minecraft:block/seagrass", seagrass),
        ] {
            let animation = registry.animation(sprite.layer).expect("the sprite animates");
            let base = registry.base_layer(animation) as usize;
            let frames = source_frames(id);
            assert_eq!(animation.count as usize, frames.len());
            for step in 0..animation.count as usize {
                assert_eq!(layer(base + step), &frames[step][..], "{id} step {step}");
            }
        }
    }

    /// A frame sits in the image as a cell of a grid running across it and then down it, which is
    /// what lets a sprite pack its frames side by side instead of in one tall column.
    #[test]
    fn a_frame_is_cut_out_of_the_grid_it_sits_in() {
        // Four 2x2 frames in a 4x4 image, each filled with its own frame number.
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
        assert!(cut(&image, 4, 2, 4).is_none(), "a frame past the last one is not there");
    }

    /// One sprite is drawn in one pass, so its opacity is the worst its frames reach: were it taken
    /// from the first frame alone, an animation that only later turns translucent would be sorted
    /// into the pass that cannot blend it.
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
