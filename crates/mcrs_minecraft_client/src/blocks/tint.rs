use std::sync::LazyLock;

use crate::columns::{BlockSource, SECTION_SIZE};
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_noise::simplex::SimplexNoise;

use crate::model::{self, Pack};

use super::Catalog;
use mcrs_minecraft_mesh::tint::BIOME_TINTS;

#[derive(serde::Deserialize)]
struct BiomeFile {
    #[serde(default)]
    temperature: f32,
    #[serde(default)]
    downfall: f32,
    effects: BiomeEffects,
}

#[derive(serde::Deserialize)]
struct BiomeEffects {
    water_color: Rgb,
    #[serde(default)]
    grass_color: Option<Rgb>,
    #[serde(default)]
    foliage_color: Option<Rgb>,
    #[serde(default)]
    dry_foliage_color: Option<Rgb>,
    #[serde(default)]
    grass_color_modifier: GrassModifier,
}

#[derive(serde::Deserialize, Default, Copy, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
enum GrassModifier {
    #[default]
    None,
    DarkForest,
    Swamp,
}

/// A biome's colour for each biome tint, as `0xRRGGBB`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BiomeTint {
    grass: Grass,
    foliage: u32,
    dry_foliage: u32,
    water: u32,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Grass {
    Color(u32),
    /// Swamp grass takes one of two colours by a noise over the ground.
    Swamp,
}

const SWAMP_DARK: u32 = 0x4c763c;
const SWAMP_LIGHT: u32 = 0x6a5d39;

static BIOME_INFO_NOISE: LazyLock<SimplexNoise> =
    LazyLock::new(|| SimplexNoise::from_random_at_origin(&mut LegacyRandom::new(2345)));

impl BiomeTint {
    fn grass_at(&self, x: i32, z: i32) -> u32 {
        match self.grass {
            Grass::Color(color) => color,
            Grass::Swamp => {
                let ground =
                    BIOME_INFO_NOISE.sample_2d(x as f64 * 0.0225, z as f64 * 0.0225, 1.0, 1.0);
                if (ground as f32) < -0.1 {
                    SWAMP_DARK
                } else {
                    SWAMP_LIGHT
                }
            }
        }
    }

    fn layers_at(&self, x: i32, z: i32) -> [u32; BIOME_TINTS.len()] {
        [
            self.grass_at(x, z),
            self.foliage,
            self.dry_foliage,
            self.water,
        ]
    }
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
    let done = catalog.tints.len();
    if done == biomes.len() {
        return;
    }
    let grass_map = noted(load_colormap(pack, "grass"), &mut catalog.failures);
    let foliage_map = noted(load_colormap(pack, "foliage"), &mut catalog.failures);
    let dry_foliage_map = noted(load_colormap(pack, "dry_foliage"), &mut catalog.failures);
    for name in &biomes[done..] {
        let Some(file) = noted(load_biome(pack, name), &mut catalog.failures) else {
            catalog.tints.push(BiomeTint {
                grass: Grass::Color(0xffffff),
                foliage: 0xffffff,
                dry_foliage: 0xffffff,
                water: 0xffffff,
            });
            continue;
        };
        let (temperature, downfall, effects) = (file.temperature, file.downfall, file.effects);
        let from_map =
            |map: &Option<Vec<u8>>| sample_colormap(map.as_deref(), temperature, downfall);
        let base_grass = effects
            .grass_color
            .map_or_else(|| from_map(&grass_map), |Rgb(c)| c);
        let grass = match effects.grass_color_modifier {
            GrassModifier::None => Grass::Color(base_grass),
            GrassModifier::DarkForest => Grass::Color(((base_grass & 0xfefefe) + 0x28340a) >> 1),
            GrassModifier::Swamp => Grass::Swamp,
        };
        catalog.tints.push(BiomeTint {
            grass,
            foliage: effects
                .foliage_color
                .map_or_else(|| from_map(&foliage_map), |Rgb(c)| c),
            dry_foliage: effects
                .dry_foliage_color
                .map_or_else(|| from_map(&dry_foliage_map), |Rgb(c)| c),
            water: effects.water_color.0,
        });
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

pub(crate) fn load_colormap(pack: &Pack, name: &str) -> Result<Vec<u8>, String> {
    let path = model::resource_path(&format!("minecraft:colormap/{name}"), "textures", "png");
    let (data, width, height) = crate::atlas::decode_png(pack.read(&path)?, &path)?;
    if width != 256 || height != 256 {
        return Err(format!(
            "{path} is {width}x{height}, and a colormap is sampled as a 256x256 grid"
        ));
    }
    Ok(data)
}

/// Vanilla's colormap lookup, in doubles as it computes it. A missing map, or a climate that
/// lands outside it, gives the magenta vanilla uses to flag the same.
pub(crate) fn sample_colormap(map: Option<&[u8]>, temperature: f32, downfall: f32) -> u32 {
    const MISSING: u32 = 0xff00ff;
    let Some(map) = map else {
        return MISSING;
    };
    let temperature = f64::from(temperature.clamp(0.0, 1.0));
    let downfall = f64::from(downfall.clamp(0.0, 1.0)) * temperature;
    let x = ((1.0 - temperature) * 255.0) as usize;
    let y = ((1.0 - downfall) * 255.0) as usize;
    let offset = (y << 8 | x) * 4;
    match map.get(offset..offset + 3) {
        Some(&[r, g, b]) => u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        _ => MISSING,
    }
}

/// An item's grass colour, where vanilla's colormap resolves it.
pub(crate) fn colormap_rgba(
    map: Option<&[u8]>,
    temperature: f32,
    downfall: f32,
) -> Option<[f32; 4]> {
    let color = sample_colormap(Some(map?), temperature, downfall);
    Some(crate::sky::rgb(color).extend(1.0).to_array())
}

pub fn tint_column(store: &impl BlockSource, tints: &[BiomeTint], column: ColumnPos) -> Vec<u8> {
    const SIZE: usize = SECTION_SIZE;
    let mut out = vec![0u8; SIZE * SIZE * 4 * BIOME_TINTS.len()];
    for z in 0..SIZE {
        for x in 0..SIZE {
            let biome = surface_biome(store, column, x, z);
            let world_x = column.x * SIZE as i32 + x as i32;
            let world_z = column.z * SIZE as i32 + z as i32;
            let layers = tints
                .get(biome as usize)
                .map_or([0xffffff; BIOME_TINTS.len()], |tint| {
                    tint.layers_at(world_x, world_z)
                });
            for (layer, color) in layers.into_iter().enumerate() {
                let offset = (layer * SIZE * SIZE + z * SIZE + x) * 4;
                out[offset..offset + 4].copy_from_slice(&[
                    (color >> 16) as u8,
                    (color >> 8) as u8,
                    color as u8,
                    255,
                ]);
            }
        }
    }
    out
}

fn surface_biome(store: &impl BlockSource, column: ColumnPos, x: usize, z: usize) -> u8 {
    let Some(extent) = store.extent() else {
        return 0;
    };
    let cell = (z / 4) * 4 + x / 4;
    for step in (0..extent.sections).rev() {
        let sy = extent.min_section_y + step as i32;
        if store.section(column.x, sy, column.z).is_some() {
            return store.biome(column.x, sy, column.z, 3 * 16 + cell);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tints_of(biome: &str) -> BiomeTint {
        let mut catalog = crate::blocks::empty();
        extend_tints(Pack::corpus(), &mut catalog, &[biome.to_string()]);
        assert!(catalog.failures.is_empty(), "{:?}", catalog.failures);
        catalog.tints[0]
    }

    #[test]
    fn a_biome_without_a_colour_of_its_own_is_tinted_from_the_colormap() {
        let plains = tints_of("minecraft:plains");
        assert_eq!(
            plains.grass,
            Grass::Color(0x91bd59),
            "plains grass is the colormap texel at its temperature and downfall"
        );
        assert_eq!(plains.foliage, 0x77ab2f);
    }

    #[test]
    fn a_biome_that_names_its_own_colour_takes_it_over_the_colormap() {
        let swamp = tints_of("minecraft:swamp");
        assert_eq!(
            swamp.foliage, 0x6a7039,
            "the swamp names its foliage colour outright"
        );
        assert_eq!(swamp.water, 0x617b64, "and its water colour");
        assert_eq!(swamp.dry_foliage, 0x7b5334, "and its dry foliage colour");
    }

    #[test]
    fn dark_forest_grass_is_its_colormap_colour_pulled_toward_a_dark_green() {
        let base = sample_colormap(
            load_colormap(Pack::corpus(), "grass").ok().as_deref(),
            0.7,
            0.8,
        );
        assert_eq!(
            tints_of("minecraft:dark_forest").grass,
            Grass::Color(((base & 0xfefefe) + 0x28340a) >> 1)
        );
    }

    #[test]
    fn swamp_grass_is_one_of_two_colours_by_where_it_grows() {
        let swamp = tints_of("minecraft:swamp");
        let seen: std::collections::BTreeSet<u32> = (0..64)
            .flat_map(|x| (0..64).map(move |z| (x * 16, z * 16)))
            .map(|(x, z)| swamp.grass_at(x, z))
            .collect();
        assert_eq!(seen, [SWAMP_DARK, SWAMP_LIGHT].into_iter().collect());
    }

    #[test]
    fn a_biome_the_pack_does_not_ship_is_reported_rather_than_quietly_defaulted() {
        let mut catalog = crate::blocks::empty();
        extend_tints(Pack::corpus(), &mut catalog, &["mcrs:nowhere".to_string()]);
        assert_eq!(catalog.failures.len(), 1);
        assert!(catalog.failures[0].starts_with("mcrs:nowhere:"));
    }
}
