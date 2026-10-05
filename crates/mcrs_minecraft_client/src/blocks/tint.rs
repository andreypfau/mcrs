use std::sync::LazyLock;

use crate::columns::{BlockSource, SECTION_SIZE};
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_noise::simplex::SimplexNoise;

use crate::model::{self, Pack};
use mcrs_minecraft_biome::{Biome, GrassColorModifier};
use mcrs_minecraft_core::codec::HexRgb;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::RegistrySet;

use super::Catalog;
use mcrs_minecraft_mesh::tint::BIOME_TINTS;

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

pub(super) fn extend_tints(
    pack: &Pack,
    catalog: &mut Catalog,
    registries: &RegistrySet,
    biomes: &[String],
) {
    let done = catalog.tints.len();
    if done == biomes.len() {
        return;
    }
    let grass_map = noted(load_colormap(pack, "grass"), &mut catalog.failures);
    let foliage_map = noted(load_colormap(pack, "foliage"), &mut catalog.failures);
    let dry_foliage_map = noted(load_colormap(pack, "dry_foliage"), &mut catalog.failures);
    let registry = registries.registry::<keys::Biome>();
    let loaded = registries.entries::<keys::Biome, Biome>();
    for name in &biomes[done..] {
        let biome = registry
            .as_ref()
            .zip(loaded.as_ref())
            .and_then(|(registry, loaded)| loaded.get(registry.get(name)?))
            .ok_or_else(|| format!("{name}: not in the loaded biome registry"));
        let tint = biome.and_then(|biome| {
            let (temperature, downfall, effects) =
                (biome.temperature, biome.downfall, &biome.effects);
            let water = effects
                .water_color
                .ok_or_else(|| format!("{name}: has no water colour"))?;
            let from_map =
                |map: &Option<Vec<u8>>| sample_colormap(map.as_deref(), temperature, downfall);
            let base_grass = effects
                .grass_color
                .map_or_else(|| from_map(&grass_map), |HexRgb(c)| c);
            let grass = match effects.grass_color_modifier {
                GrassColorModifier::None => Grass::Color(base_grass),
                GrassColorModifier::DarkForest => {
                    Grass::Color(((base_grass & 0xfefefe) + 0x28340a) >> 1)
                }
                GrassColorModifier::Swamp => Grass::Swamp,
            };
            Ok(BiomeTint {
                grass,
                foliage: effects
                    .foliage_color
                    .map_or_else(|| from_map(&foliage_map), |HexRgb(c)| c),
                dry_foliage: effects
                    .dry_foliage_color
                    .map_or_else(|| from_map(&dry_foliage_map), |HexRgb(c)| c),
                water: water.0,
            })
        });
        catalog
            .tints
            .push(noted(tint, &mut catalog.failures).unwrap_or(BiomeTint {
                grass: Grass::Color(0xffffff),
                foliage: 0xffffff,
                dry_foliage: 0xffffff,
                water: 0xffffff,
            }));
    }
}

fn noted<T>(result: Result<T, String>, failures: &mut Vec<String>) -> Option<T> {
    result.map_err(|reason| failures.push(reason)).ok()
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
    let cell = (x, 15, z);
    for step in (0..extent.sections).rev() {
        let sy = extent.min_section_y + step as i32;
        if store.section(column.x, sy, column.z).is_some() {
            return store.biome(column.x, sy, column.z, cell);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::columns::{Column, ColumnStore, Extent, SECTION_VOLUME, Section};
    use mcrs_minecraft_chunk::section::Biomes;
    use mcrs_minecraft_chunk::{PalettedContainer, SectionKind};
    use mcrs_minecraft_world::registries::test_registries;

    fn tints_of(biome: &str) -> BiomeTint {
        let mut catalog = crate::blocks::empty();
        extend_tints(
            Pack::corpus(),
            &mut catalog,
            test_registries(),
            &[biome.to_string()],
        );
        assert!(catalog.failures.is_empty(), "{:?}", catalog.failures);
        catalog.tints[0]
    }

    #[test]
    fn tints_come_from_the_loaded_biome_column() {
        let names = ["plains", "swamp", "beta_desert"].map(|name| format!("minecraft:{name}"));
        let mut catalog = crate::blocks::empty();
        extend_tints(Pack::corpus(), &mut catalog, test_registries(), &names);
        assert!(catalog.failures.is_empty(), "{:?}", catalog.failures);
        assert_eq!(
            catalog.tints,
            [
                BiomeTint {
                    grass: Grass::Color(0x91bd59),
                    foliage: 0x77ab2f,
                    dry_foliage: 0xa37546,
                    water: 0x3f76e4,
                },
                BiomeTint {
                    grass: Grass::Swamp,
                    foliage: 0x6a7039,
                    dry_foliage: 0x7b5334,
                    water: 0x617b64,
                },
                BiomeTint {
                    grass: Grass::Color(0xbfb755),
                    foliage: 0xaea42a,
                    dry_foliage: 0xa38046,
                    water: 0x3f76e4,
                },
            ]
        );
    }

    #[test]
    fn biome_tints_read_the_resource_pack() {
        a_biome_without_a_colour_of_its_own_is_tinted_from_the_colormap();
        a_biome_that_names_its_own_colour_takes_it_over_the_colormap();
        dark_forest_grass_is_its_colormap_colour_pulled_toward_a_dark_green();
        swamp_grass_is_one_of_two_colours_by_where_it_grows();
        a_biome_the_pack_does_not_ship_is_reported_rather_than_quietly_defaulted();
    }

    fn a_biome_without_a_colour_of_its_own_is_tinted_from_the_colormap() {
        let plains = tints_of("minecraft:plains");
        assert_eq!(
            plains.grass,
            Grass::Color(0x91bd59),
            "plains grass is the colormap texel at its temperature and downfall"
        );
        assert_eq!(plains.foliage, 0x77ab2f);
    }

    fn a_biome_that_names_its_own_colour_takes_it_over_the_colormap() {
        let swamp = tints_of("minecraft:swamp");
        assert_eq!(
            swamp.foliage, 0x6a7039,
            "the swamp names its foliage colour outright"
        );
        assert_eq!(swamp.water, 0x617b64, "and its water colour");
        assert_eq!(swamp.dry_foliage, 0x7b5334, "and its dry foliage colour");
    }

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

    fn swamp_grass_is_one_of_two_colours_by_where_it_grows() {
        let swamp = tints_of("minecraft:swamp");
        let seen: std::collections::BTreeSet<u32> = (0..64)
            .flat_map(|x| (0..64).map(move |z| (x * 16, z * 16)))
            .map(|(x, z)| swamp.grass_at(x, z))
            .collect();
        assert_eq!(seen, [SWAMP_DARK, SWAMP_LIGHT].into_iter().collect());
    }

    fn section_holding(biomes: PalettedContainer<u8, { Biomes::SIZE }>) -> Section {
        Section {
            blocks: Box::new([0; SECTION_VOLUME]),
            biomes,
            states: vec![0],
        }
    }

    #[test]
    fn the_tint_reads_the_biome_of_the_top_block_of_the_highest_section() {
        let mut cells = vec![2u8; Biomes::ENTRY_COUNT];
        cells[Biomes::index(3, 15, 7)] = 1;
        cells[Biomes::index(3, 0, 7)] = 5;
        let highest = section_holding(PalettedContainer::from_cells(&cells));
        let lower = section_holding(PalettedContainer::Homogeneous(3));

        let extent = Extent {
            min_section_y: 0,
            sections: 3,
        };
        let column = ColumnPos::new(0, 0);
        let mut store = ColumnStore::default();
        store.enter(extent);
        store.insert(
            column,
            Column::unlit(0, vec![Some(lower), Some(highest), None]),
        );

        assert_eq!(surface_biome(&store, column, 3, 7), 1);
        assert_eq!(surface_biome(&store, column, 4, 7), 2);
    }

    fn a_biome_the_pack_does_not_ship_is_reported_rather_than_quietly_defaulted() {
        let mut catalog = crate::blocks::empty();
        extend_tints(
            Pack::corpus(),
            &mut catalog,
            test_registries(),
            &["mcrs:nowhere".to_string()],
        );
        assert_eq!(catalog.failures.len(), 1);
        assert!(catalog.failures[0].starts_with("mcrs:nowhere:"));
    }
}
