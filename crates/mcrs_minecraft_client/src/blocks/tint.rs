use crate::anvil::{REGION_BLOCKS, SECTION_SIZE, World};
use crate::model::{self, Pack};

use super::{Catalog, TINT_KINDS};

#[derive(serde::Deserialize)]
struct BiomeFile {
    #[serde(default)]
    temperature: f32,
    #[serde(default)]
    downfall: f32,
    #[serde(default)]
    effects: BiomeEffects,
}

#[derive(serde::Deserialize, Default)]
struct BiomeEffects {
    #[serde(default)]
    water_color: Option<Rgb>,
    #[serde(default)]
    grass_color: Option<Rgb>,
    #[serde(default)]
    foliage_color: Option<Rgb>,
}

/// A biome colour is written either as `#rrggbb` or as the packed integer that spells.
#[derive(Copy, Clone)]
struct Rgb(u32);

impl<'de> serde::Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Either;

        impl serde::de::Visitor<'_> for Either {
            type Value = Rgb;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a `#rrggbb` colour or the packed integer that spells it")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Rgb, E> {
                let digits = value
                    .strip_prefix('#')
                    .filter(|digits| digits.len() == 6)
                    .ok_or_else(|| E::custom(format!("`{value}` is not a `#rrggbb` colour")))?;
                u32::from_str_radix(digits, 16)
                    .map(Rgb)
                    .map_err(|error| E::custom(format!("`{value}`: {error}")))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Rgb, E> {
                Ok(Rgb(value as u32))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Rgb, E> {
                Ok(Rgb(value as u32))
            }
        }

        deserializer.deserialize_any(Either)
    }
}

pub(super) fn extend_tints(pack: &Pack, catalog: &mut Catalog, biomes: &[String]) {
    let done = (catalog.tints.len() - 1) / TINT_KINDS;
    if done == biomes.len() {
        return;
    }
    let grass_map = noted(load_colormap(pack, "grass"), &mut catalog.failures);
    let foliage_map = noted(load_colormap(pack, "foliage"), &mut catalog.failures);
    for name in &biomes[done..] {
        let file = noted(load_biome(pack, name), &mut catalog.failures);
        let (temperature, downfall, effects) = match file {
            Some(file) => (file.temperature, file.downfall, file.effects),
            None => (0.5, 0.5, BiomeEffects::default()),
        };
        let grass = effects
            .grass_color
            .map(rgb)
            .or_else(|| sample_colormap(grass_map.as_deref(), temperature, downfall))
            .unwrap_or([0.56, 0.73, 0.35, 1.0]);
        let foliage = effects
            .foliage_color
            .map(rgb)
            .or_else(|| sample_colormap(foliage_map.as_deref(), temperature, downfall))
            .unwrap_or([0.29, 0.60, 0.21, 1.0]);
        let water = effects
            .water_color
            .map(rgb)
            .unwrap_or([0.25, 0.46, 0.89, 1.0]);
        catalog.tints.extend_from_slice(&[grass, foliage, water]);
    }
}

fn noted<T>(result: Result<T, String>, failures: &mut Vec<String>) -> Option<T> {
    result.map_err(|reason| failures.push(reason)).ok()
}

fn load_biome(pack: &Pack, name: &str) -> Result<BiomeFile, String> {
    let path = model::resource_path(name, "worldgen/biome", "json");
    let bytes = pack
        .read(&path)
        .map_err(|reason| format!("{name}: {reason}"))?;
    serde_json::from_slice(bytes).map_err(|error| format!("{name}: cannot parse {path}: {error}"))
}

fn load_colormap(pack: &Pack, name: &str) -> Result<Vec<u8>, String> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    use bevy::prelude::Image;

    let path = model::resource_path(&format!("minecraft:colormap/{name}"), "textures", "png");
    let image = Image::from_buffer(
        pack.read(&path)?,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .map_err(|error| format!("cannot decode {path}: {error}"))?;
    if image.width() != 256 || image.height() != 256 {
        return Err(format!(
            "{path} is {}x{}, and a colormap is sampled as a 256x256 grid",
            image.width(),
            image.height(),
        ));
    }
    image
        .data
        .ok_or_else(|| format!("{path} decoded without pixel data"))
}

fn sample_colormap(map: Option<&[u8]>, temperature: f32, downfall: f32) -> Option<[f32; 4]> {
    let map = map?;
    let t = temperature.clamp(0.0, 1.0);
    let d = downfall.clamp(0.0, 1.0) * t;
    let column = ((1.0 - t) * 255.0) as usize;
    let row = ((1.0 - d) * 255.0) as usize;
    let offset = (row * 256 + column) * 4;
    if offset + 3 >= map.len() {
        return None;
    }
    Some([
        map[offset] as f32 / 255.0,
        map[offset + 1] as f32 / 255.0,
        map[offset + 2] as f32 / 255.0,
        1.0,
    ])
}

fn rgb(Rgb(packed): Rgb) -> [f32; 4] {
    [
        ((packed >> 16) & 0xff) as f32 / 255.0,
        ((packed >> 8) & 0xff) as f32 / 255.0,
        (packed & 0xff) as f32 / 255.0,
        1.0,
    ]
}

pub fn tint_square(world: &World, tints: &[[f32; 4]], corner: [usize; 2]) -> Vec<u8> {
    const SIZE: usize = REGION_BLOCKS;
    let mut out = vec![0u8; SIZE * SIZE * 4 * TINT_KINDS];
    for z in 0..SIZE {
        for x in 0..SIZE {
            let biome = surface_biome(world, corner[0] + x, corner[1] + z);
            for kind in 0..TINT_KINDS {
                let slot = 1 + biome as usize * TINT_KINDS + kind;
                let color = tints.get(slot).copied().unwrap_or([1.0; 4]);
                let offset = (kind * SIZE * SIZE + z * SIZE + x) * 4;
                for channel in 0..4 {
                    out[offset + channel] = (color[channel].clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }
    }
    out
}

fn surface_biome(world: &World, x: usize, z: usize) -> u8 {
    let sx = x / SECTION_SIZE;
    let sz = z / SECTION_SIZE;
    let cell = ((z % SECTION_SIZE) / 4) * 4 + (x % SECTION_SIZE) / 4;
    for sy in (0..world.sections[1]).rev() {
        if world.section(sx, sy, sz).is_some() {
            return world.biome(sx, sy, sz, 3 * 16 + cell);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_biome_without_a_colour_of_its_own_is_tinted_from_the_colormap() {
        let mut catalog = crate::blocks::empty();
        extend_tints(
            Pack::corpus(),
            &mut catalog,
            &["minecraft:plains".to_string()],
        );
        assert!(catalog.failures.is_empty(), "{:?}", catalog.failures);
        assert_eq!(
            catalog.tints[1],
            [145.0 / 255.0, 189.0 / 255.0, 89.0 / 255.0, 1.0],
            "plains grass is the colormap texel at its temperature and downfall"
        );
    }

    #[test]
    fn a_biome_that_names_its_own_colour_takes_it_over_the_colormap() {
        let mut catalog = crate::blocks::empty();
        extend_tints(
            Pack::corpus(),
            &mut catalog,
            &["minecraft:swamp".to_string()],
        );
        assert!(catalog.failures.is_empty(), "{:?}", catalog.failures);
        assert_eq!(
            catalog.tints[2],
            [
                0x6a as f32 / 255.0,
                0x70 as f32 / 255.0,
                0x39 as f32 / 255.0,
                1.0
            ],
            "the swamp names its foliage colour outright"
        );
        assert_eq!(
            catalog.tints[3],
            [
                0x61 as f32 / 255.0,
                0x7b as f32 / 255.0,
                0x64 as f32 / 255.0,
                1.0
            ],
            "and its water colour"
        );
    }

    #[test]
    fn a_biome_the_pack_does_not_ship_is_reported_rather_than_quietly_defaulted() {
        let mut catalog = crate::blocks::empty();
        extend_tints(Pack::corpus(), &mut catalog, &["mcrs:nowhere".to_string()]);
        assert_eq!(catalog.failures.len(), 1);
        assert!(catalog.failures[0].starts_with("mcrs:nowhere:"));
    }
}
