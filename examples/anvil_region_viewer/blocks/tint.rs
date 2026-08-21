use crate::anvil::{REGION_BLOCKS, SECTION_SIZE, World};
use crate::model;

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
    water_color: Option<u32>,
    #[serde(default)]
    grass_color: Option<u32>,
    #[serde(default)]
    foliage_color: Option<u32>,
}

pub(super) fn extend_tints(catalog: &mut Catalog, biomes: &[String]) {
    let done = (catalog.tints.len() - 1) / TINT_KINDS;
    if done == biomes.len() {
        return;
    }
    let grass_map = load_colormap("grass");
    let foliage_map = load_colormap("foliage");
    for name in &biomes[done..] {
        let file = load_biome(name);
        let (temperature, downfall, effects) = match file {
            Some(file) => (file.temperature, file.downfall, file.effects),
            None => (0.5, 0.5, BiomeEffects::default()),
        };
        let grass = effects
            .grass_color
            .map(rgb)
            .or_else(|| sample_colormap(&grass_map, temperature, downfall))
            .unwrap_or([0.56, 0.73, 0.35, 1.0]);
        let foliage = effects
            .foliage_color
            .map(rgb)
            .or_else(|| sample_colormap(&foliage_map, temperature, downfall))
            .unwrap_or([0.29, 0.60, 0.21, 1.0]);
        let water = effects.water_color.map(rgb).unwrap_or([0.25, 0.46, 0.89, 1.0]);
        catalog.tints.extend_from_slice(&[grass, foliage, water]);
    }
}

fn load_biome(name: &str) -> Option<BiomeFile> {
    let path = model::resource_path(name, "worldgen/biome", "json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn load_colormap(name: &str) -> Option<Vec<u8>> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    use bevy::prelude::Image;

    let path = model::resource_path(
        &format!("minecraft:colormap/{name}"),
        "textures",
        "png",
    );
    let bytes = std::fs::read(path).ok()?;
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .ok()?;
    if image.width() != 256 || image.height() != 256 {
        return None;
    }
    image.data
}

fn sample_colormap(map: &Option<Vec<u8>>, temperature: f32, downfall: f32) -> Option<[f32; 4]> {
    let map = map.as_ref()?;
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

fn rgb(packed: u32) -> [f32; 4] {
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
