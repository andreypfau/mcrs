use std::collections::HashMap;

use mcrs_minecraft_biome::climate::ClimateParameters;
use mcrs_minecraft_biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_biome::zoom::{obfuscate_seed, quart_cell};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_worldgen_density::program::Workspace;

use super::build_settings_router;
use crate::modern_carvers::climate_target_at;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::{multi_noise_grid, multi_noise_palettes};
use bevy_math::IVec3;

/// The preset's biomes numbered in the order the preset names them, which is
/// all a palette needs of a registry: distinct ids that round-trip.
pub(super) fn preset_ids() -> HashMap<String, u8> {
    let mut ids = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = ids.len() as u8;
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    ids
}

pub(super) fn overworld_table() -> (MultiNoiseBiomeTable, HashMap<String, u8>) {
    let ids = preset_ids();
    let source = MultiNoiseBiomeSource {
        preset: Some(mcrs_minecraft_core::ResourceLocation::parse("minecraft:overworld").unwrap()),
        biomes: None,
    };
    let table = MultiNoiseBiomeTable::resolve(&source, |biome| {
        Some(*ids.get(biome).expect("the preset names its own biome"))
    })
    .expect("the overworld preset resolves");
    (table, ids)
}

fn y_sections() -> Vec<i32> {
    (-4..20).collect()
}

fn a_column_carries_its_cave_biome_under_its_surface_biome(
    router: &mcrs_minecraft_worldgen_density::router::NoiseRouter,
    table: &MultiNoiseBiomeTable,
    ids: &HashMap<String, u8>,
) {
    let sections = y_sections();

    let palettes = multi_noise_palettes(router, table, 0, 0, &sections);
    let grid = multi_noise_grid(router, table, 0, 0, &sections)
        .expect("the multi-noise path builds a grid");
    let first = sections[0];
    let column: Vec<u8> = sections
        .iter()
        .map(|&section_y| grid.get(1, (section_y - first) * 4, 1))
        .collect();
    let distinct: std::collections::BTreeSet<u8> = column.iter().copied().collect();
    let mut stored = std::collections::BTreeSet::new();
    for palette in &palettes {
        palette.for_each_distinct(|biome| {
            stored.insert(biome);
        });
    }

    let name_of = |biome: &str| *ids.get(biome).expect("the preset names it");
    assert_eq!(
        distinct,
        [
            name_of("minecraft:forest"),
            name_of("minecraft:dripstone_caves")
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<u8>>(),
        "the column should hold a surface biome over a cave biome"
    );
    assert!(
        stored.contains(&name_of("minecraft:forest"))
            && stored.contains(&name_of("minecraft:dripstone_caves")),
        "the stored column should hold both: {stored:?}"
    );
}

/// A source that lists its biomes rather than naming a preset.
#[test]
fn an_explicit_entry_list_resolves_by_location() {
    use mcrs_minecraft_biome::climate::ParameterRange;

    let flat = |value: f64| ClimateParameters {
        temperature: ParameterRange::Point(value),
        humidity: ParameterRange::Point(0.0),
        continentalness: ParameterRange::Point(0.0),
        erosion: ParameterRange::Point(0.0),
        depth: ParameterRange::Point(0.0),
        weirdness: ParameterRange::Point(0.0),
        offset: 0.0,
    };
    let source = MultiNoiseBiomeSource {
        preset: None,
        biomes: Some(vec![
            entry(flat(-1.0), "minecraft:plains"),
            entry(flat(1.0), "minecraft:desert"),
        ]),
    };
    let table = MultiNoiseBiomeTable::resolve(&source, |biome| match biome {
        "minecraft:plains" => Some(7),
        "minecraft:desert" => Some(9),
        other => panic!("unexpected biome {other}"),
    })
    .expect("an explicit list resolves");
    assert_eq!(table.len(), 2);
    assert_eq!(
        table.biome_at(mcrs_minecraft_biome::climate::TargetPoint::new(
            -1.0, 0.0, 0.0, 0.0, 0.0, 0.0
        )),
        7
    );
    assert_eq!(
        table.biome_at(mcrs_minecraft_biome::climate::TargetPoint::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0
        )),
        9
    );
}

fn entry(
    parameters: ClimateParameters,
    biome: &str,
) -> mcrs_minecraft_biome::source::MultiNoiseBiomeEntry {
    mcrs_minecraft_biome::source::MultiNoiseBiomeEntry {
        parameters,
        biome: bevy_asset::Handle::default(),
        location: mcrs_minecraft_core::ResourceLocation::parse(biome).unwrap(),
    }
}

/// The palette stores a biome in a byte. A registry that grew past 256 entries
/// would narrow an id into a different biome's, which nothing downstream can
/// see, so the table refuses to be built at all.
#[test]
fn a_biome_whose_id_does_not_fit_a_byte_refuses_the_table() {
    let ids = preset_ids();
    let source = MultiNoiseBiomeSource {
        preset: Some(mcrs_minecraft_core::ResourceLocation::parse("minecraft:overworld").unwrap()),
        biomes: None,
    };
    let oversized = "minecraft:eroded_badlands";
    assert!(ids.contains_key(oversized), "the preset names it");

    let table = MultiNoiseBiomeTable::resolve(&source, |biome| {
        if biome == oversized {
            u8::try_from(260u32).ok()
        } else {
            Some(*ids.get(biome).expect("the preset names its own biome"))
        }
    });
    assert!(table.is_none());
}

#[test]
fn an_unknown_preset_is_not_resolved() {
    let source = MultiNoiseBiomeSource {
        preset: Some(mcrs_minecraft_core::ResourceLocation::parse("minecraft:the_end").unwrap()),
        biomes: None,
    };
    assert!(MultiNoiseBiomeTable::resolve(&source, |_| Some(0)).is_none());
}

/// The zoom reads eight quart corners around a block and they reach outside the
/// column horizontally, so the grid carries a ring of cells the palette does
/// not store; vertically the grid is the column, and the palette comes from its
/// cells.
#[test]
fn the_grid_rings_the_column_by_one_quart_cell() {
    let router = build_settings_router("overworld", 2);
    let (table, ids) = overworld_table();
    a_column_carries_its_cave_biome_under_its_surface_biome(&router, &table, &ids);
    let sections = y_sections();
    let (chunk_x, chunk_z) = (26, 90);
    let first = sections[0];

    let palettes = multi_noise_palettes(&router, &table, chunk_x * 16, chunk_z * 16, &sections);
    let grid = multi_noise_grid(&router, &table, chunk_x * 16, chunk_z * 16, &sections)
        .expect("the multi-noise path builds a grid");
    assert_eq!(
        grid.volume.size(),
        IVec3::new(6, sections.len() as i32 * 4, 6)
    );
    assert_eq!(
        grid.volume.min_block(),
        IVec3::new(chunk_x * 16 - 4, first * 16, chunk_z * 16 - 4)
    );
    assert!(
        palettes.iter().any(|palette| {
            let mut distinct = 0;
            palette.for_each_distinct(|_| distinct += 1);
            distinct > 1
        }),
        "a column with a section of several biomes"
    );

    let zoom_seed = obfuscate_seed(router.world_seed as i64);
    let min = grid.volume.min_block();
    let origin = IVec3::new(min.x >> 2, min.y >> 2, min.z >> 2);
    let (first_row, last_row) = (first * 4, sections[sections.len() - 1] * 4 + 3);
    let plain_pick = |block: BlockPos| {
        let quart = quart_cell(zoom_seed, block);
        let row = quart.y.clamp(first_row, last_row) - origin.y;
        grid.get(quart.x - origin.x, row, quart.z - origin.z)
    };
    for (index, &section_y) in sections.iter().enumerate() {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let block = BlockPos::new(
                        chunk_x * 16 + x as i32,
                        section_y * 16 + y as i32,
                        chunk_z * 16 + z as i32,
                    );
                    assert_eq!(
                        palettes[index].get_cell(x, y, z),
                        plain_pick(block),
                        "block {x},{y},{z} of section {section_y}"
                    );
                }
            }
        }
    }

    let mut ws = Workspace::new();
    for (gx, gy, gz) in [(0, 0, 0), (5, grid.volume.size().y - 1, 5), (2, 7, 5)] {
        let target = climate_target_at(
            &router,
            &mut ws,
            grid.volume.block_x(gx) >> 2,
            grid.volume.block_y(gy) >> 2,
            grid.volume.block_z(gz) >> 2,
        );
        assert_eq!(
            grid.get(gx, gy, gz),
            table.biome_at(target),
            "grid cell {gx},{gy},{gz}"
        );
    }
}
