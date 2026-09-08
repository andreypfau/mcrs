use std::collections::HashMap;

use mcrs_minecraft_world::biome::climate::ClimateParameters;
use mcrs_minecraft_world::biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_world::biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_worldgen::program::Workspace;

use super::build_settings_router;
use crate::world::generate::modern_carvers::climate_target_at;
use crate::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::world::generate::multi_noise_palettes;
use bevy_math::IVec3;

/// The preset's biomes numbered in the order the preset names them, which is
/// all a palette needs of a registry: distinct ids that round-trip.
fn preset_ids() -> HashMap<String, u8> {
    let mut ids = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = ids.len() as u8;
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    ids
}

fn overworld_table() -> (MultiNoiseBiomeTable, HashMap<String, u8>) {
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

#[test]
fn the_overworld_preset_resolves_every_entry() {
    let (table, ids) = overworld_table();
    assert_eq!(table.len(), overworld_parameter_list().len());
    // The preset is a fan of many biomes over one climate space, not one biome
    // repeated: a table that collapsed to a single id would still fill palettes.
    assert!(ids.len() > 20, "only {} distinct biomes", ids.len());
}

/// The batched volume is the only thing standing between a cell and its
/// climate, so it must answer exactly what sampling that cell alone answers.
#[test]
fn the_batched_column_agrees_with_sampling_each_cell() {
    let router = build_settings_router("overworld", 2);
    let (table, _) = overworld_table();
    let sections = y_sections();
    let (chunk_x, chunk_z) = (26, 90);

    let (palettes, _) =
        multi_noise_palettes(&router, &table, chunk_x * 16, chunk_z * 16, &sections);
    assert_eq!(palettes.len(), sections.len());

    let mut ws = Workspace::new();
    for (index, &section_y) in sections.iter().enumerate() {
        for cx in 0..4 {
            for cy in 0..4 {
                for cz in 0..4 {
                    let target = climate_target_at(
                        &router,
                        &mut ws,
                        chunk_x * 4 + cx,
                        section_y * 4 + cy,
                        chunk_z * 4 + cz,
                    );
                    assert_eq!(
                        palettes[index].get_cell(cx as usize, cy as usize, cz as usize),
                        table.biome_at(target),
                        "cell {cx},{cy},{cz} of section {section_y}"
                    );
                }
            }
        }
    }
}

/// A modern biome is a function of Y as much as of X and Z, so the column must
/// not come out as one palette cloned down its sections. At the origin the
/// surface is forest and the cave layer is dripstone.
#[test]
fn a_column_carries_its_cave_biome_under_its_surface_biome() {
    let router = build_settings_router("overworld", 2);
    let (table, ids) = overworld_table();
    let sections = y_sections();

    let (palettes, _) = multi_noise_palettes(&router, &table, 0, 0, &sections);
    let column: Vec<u8> = palettes.iter().map(|p| p.get_cell(0, 0, 0)).collect();
    let distinct: std::collections::BTreeSet<u8> = column.iter().copied().collect();

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
}

/// A source that lists its biomes rather than naming a preset.
#[test]
fn an_explicit_entry_list_resolves_by_location() {
    use mcrs_minecraft_world::biome::climate::ParameterRange;

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
        table.biome_at(mcrs_minecraft_world::biome::climate::TargetPoint::new(
            -1.0, 0.0, 0.0, 0.0, 0.0, 0.0
        )),
        7
    );
    assert_eq!(
        table.biome_at(mcrs_minecraft_world::biome::climate::TargetPoint::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0
        )),
        9
    );
}

fn entry(
    parameters: ClimateParameters,
    biome: &str,
) -> mcrs_minecraft_world::biome::source::MultiNoiseBiomeEntry {
    mcrs_minecraft_world::biome::source::MultiNoiseBiomeEntry {
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

#[test]
#[ignore = "measurement, not an assertion"]
fn measure_multi_noise_palettes() {
    let router = build_settings_router("overworld", 2);
    let (table, _) = overworld_table();
    let sections = y_sections();
    let _ = multi_noise_palettes(&router, &table, 0, 0, &sections);
    let columns = 64;
    let started = std::time::Instant::now();
    for i in 0..columns {
        std::hint::black_box(multi_noise_palettes(
            &router,
            &table,
            i * 16,
            (i % 7) * 16,
            &sections,
        ));
    }
    let each = started.elapsed().as_secs_f64() * 1000.0 / columns as f64;
    println!("MEASURE multi-noise biomes {each:.3} ms/column");
}

/// The zoom reads eight quart corners around a block and they reach outside the
/// column on all three axes, so the grid carries a ring of cells the palette
/// does not store — and the palette must still come from the cells it used to.
#[test]
fn the_grid_rings_the_column_by_one_quart_cell() {
    let router = build_settings_router("overworld", 2);
    let (table, _) = overworld_table();
    let sections = y_sections();
    let (chunk_x, chunk_z) = (26, 90);
    let first = sections[0];

    let (palettes, grid) =
        multi_noise_palettes(&router, &table, chunk_x * 16, chunk_z * 16, &sections);
    let grid = grid.expect("the multi-noise path builds a grid");
    assert_eq!(
        grid.volume.size(),
        IVec3::new(6, sections.len() as i32 * 4 + 2, 6)
    );
    assert_eq!(
        grid.volume.min_block(),
        IVec3::new(chunk_x * 16 - 4, first * 16 - 4, chunk_z * 16 - 4)
    );

    for (index, &section_y) in sections.iter().enumerate() {
        for cx in 0..4 {
            for cy in 0..4 {
                for cz in 0..4 {
                    assert_eq!(
                        grid.get(cx + 1, (section_y - first) * 4 + cy + 1, cz + 1),
                        palettes[index].get_cell(cx as usize, cy as usize, cz as usize),
                        "cell {cx},{cy},{cz} of section {section_y}"
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
