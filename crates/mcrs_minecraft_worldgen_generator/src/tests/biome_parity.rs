use bytes::Buf;
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};
use std::path::PathBuf;

use super::build_settings_router;
use super::multi_noise_biomes::overworld_table;
use crate::multi_noise_palettes;

const MAGIC: &[u8; 8] = b"MCBIOME0";
const BLOCKS: usize = 16 * 16 * 16;

struct Column {
    seed: u64,
    chunk_x: i32,
    chunk_z: i32,
    min_section: i32,
    palette: Vec<String>,
    sections: Vec<Vec<u32>>,
}

impl Column {
    fn section_ys(&self) -> Vec<i32> {
        (self.min_section..self.min_section + self.sections.len() as i32).collect()
    }

    fn at(&self, section: usize, x: usize, y: usize, z: usize) -> &str {
        &self.palette[self.sections[section][(y * 16 + z) * 16 + x] as usize]
    }
}

fn read_dump() -> Vec<Column> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/biome_containers.bin");
    let mut r = open_dump(&path, MAGIC);
    let columns = (0..r.get_u32_le())
        .map(|_| {
            let seed = r.get_i64_le() as u64;
            let chunk_x = r.get_i32_le();
            let chunk_z = r.get_i32_le();
            let min_section = r.get_i32_le();
            let section_count = r.get_u32_le();
            let palette: Vec<String> = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
            let sections = (0..section_count)
                .map(|_| {
                    let mut cells = Vec::with_capacity(BLOCKS);
                    for _ in 0..r.get_u32_le() {
                        let (id, run) = (r.get_u32_le(), r.get_u32_le());
                        cells.extend(std::iter::repeat_n(id, run as usize));
                    }
                    assert_eq!(
                        cells.len(),
                        BLOCKS,
                        "a section of chunk {chunk_x},{chunk_z} is short"
                    );
                    cells
                })
                .collect();
            Column {
                seed,
                chunk_x,
                chunk_z,
                min_section,
                palette,
                sections,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in the biome dump");
    columns
}

#[test]
#[ignore = "reference parity check; run with --ignored"]
fn the_stored_biomes_match_the_reference_block_for_block() {
    let (table, ids) = overworld_table();
    let mut names = vec![""; ids.len()];
    for (name, &id) in &ids {
        names[id as usize] = name;
    }

    let columns = read_dump();
    let mut checked = 0usize;
    for column in &columns {
        let router = build_settings_router("overworld", column.seed);
        let sections = column.section_ys();
        let containers = multi_noise_palettes(
            &router,
            &table,
            column.chunk_x * 16,
            column.chunk_z * 16,
            &sections,
        );
        assert_eq!(containers.len(), sections.len());

        for (index, container) in containers.iter().enumerate() {
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let want = column.at(index, x, y, z);
                        let got = names[container.get_cell(x, y, z) as usize];
                        assert_eq!(
                            got, want,
                            "seed {} chunk {},{} section {} block {x},{y},{z}",
                            column.seed, column.chunk_x, column.chunk_z, sections[index],
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(
        checked,
        columns
            .iter()
            .map(|c| c.sections.len() * BLOCKS)
            .sum::<usize>(),
        "a block went uncompared"
    );
}

#[test]
#[ignore = "reference parity check; run with --ignored"]
fn the_dump_holds_a_section_with_more_than_one_biome() {
    let columns = read_dump();

    let mixed_section = columns.iter().any(|column| {
        column
            .sections
            .iter()
            .any(|cells| cells.iter().any(|&id| id != cells[0]))
    });
    assert!(mixed_section, "every section of the dump is one biome");

    let mixed_layer = columns.iter().any(|column| {
        column.sections.iter().any(|cells| {
            cells
                .chunks(16 * 16)
                .any(|layer| layer.iter().any(|&id| id != layer[0]))
        })
    });
    assert!(
        mixed_layer,
        "no horizontal layer of the dump holds two biomes"
    );
}
