use std::collections::HashMap;

use mcrs_minecraft_biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_biome::zoom::{FiddleCache, obfuscate_seed, quart_cell};
use mcrs_minecraft_core::BlockPos;

use super::build_settings_router;
use crate::biome_upscale::upscale_biomes;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::multi_noise_palettes;

fn overworld_table() -> MultiNoiseBiomeTable {
    let mut ids: HashMap<String, u8> = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = ids.len() as u8;
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    let source = MultiNoiseBiomeSource {
        preset: Some(mcrs_minecraft_core::ResourceLocation::parse("minecraft:overworld").unwrap()),
        biomes: None,
    };
    MultiNoiseBiomeTable::resolve(&source, |biome| Some(ids[biome]))
        .expect("the overworld preset resolves")
}

#[test]
fn the_upscale_equals_the_plain_zoom_at_every_block() {
    let router = build_settings_router("overworld", 2);
    let table = overworld_table();
    let sections: Vec<i32> = (-4..20).collect();
    let zoom_seed = obfuscate_seed(2);

    let mut found = None;
    for chunk_x in 0..64 {
        let (_, grid) = multi_noise_palettes(&router, &table, chunk_x * 16, 0, &sections);
        let grid = grid.expect("the multi-noise path builds a grid");
        let containers = upscale_biomes(&grid, zoom_seed, &sections, &mut FiddleCache::default());
        let mixed = containers.iter().any(|container| {
            let mut distinct = 0;
            container.for_each_distinct(|_| distinct += 1);
            distinct > 1
        });
        if mixed {
            found = Some((chunk_x, grid, containers));
            break;
        }
    }
    let (chunk_x, grid, containers) = found.expect("a column with a section of several biomes");
    assert_eq!(containers.len(), sections.len());

    let min = grid.volume.min_block();
    let rows = grid.volume.size().y;
    let expected = |quart: mcrs_minecraft_core::QuartPos| {
        let x = quart.x - (min.x >> 2);
        let z = quart.z - (min.z >> 2);
        let y = (quart.y - (min.y >> 2)).clamp(1, rows - 2);
        grid.get(x, y, z)
    };

    for (container, &section_y) in containers.iter().zip(&sections) {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let pos = BlockPos::new(chunk_x * 16 + x, section_y * 16 + y, z);
                    assert_eq!(
                        container.get_cell(x as usize, y as usize, z as usize),
                        expected(quart_cell(zoom_seed, pos)),
                        "block {x},{y},{z} of section {section_y} at chunk {chunk_x}"
                    );
                }
            }
        }
    }
}
