use mcrs_minecraft_worldgen_testing::worldgen_dir;
use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_value_provider::HeightContext;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::aquifer::point_barrier;
use mcrs_minecraft_worldgen_density::program::Workspace;

use super::{build_settings_router, corpus};
use crate::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, apply_modern_carvers, climate_target_at,
};
use crate::{ColumnBlocks, column_fluid_field};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::parameter_list::Preset;

fn overworld_height() -> HeightContext {
    HeightContext {
        min_y: -64,
        depth: 384,
        sea_level: 63,
    }
}

fn y_sections() -> Vec<i32> {
    (-4..20).collect()
}

/// The carver list a biome actually ships, read the way the loader would.
pub(super) fn carvers_of(biome: &str) -> Arc<[CarverConfig]> {
    let id = mcrs_minecraft_core::ResourceLocation::read(biome).expect("a biome id");
    let biome: mcrs_minecraft_biome_file::BiomeFile =
        mcrs_minecraft_worldgen_testing::read("biome", &id);
    mcrs_minecraft_worldgen_testing::names_of(&biome.carvers)
        .iter()
        .map(|name| {
            let id = mcrs_minecraft_core::ResourceLocation::read(name).expect("a carver id");
            mcrs_minecraft_worldgen_testing::read("carver", &id)
        })
        .collect()
}

fn stone_column() -> (ColumnBlocks, VoxelId, VoxelId) {
    let sections = y_sections();
    let column = ColumnBlocks::new(&sections);
    let stone: VoxelId = corpus().default_state("minecraft:stone").0.into();
    let bedrock: VoxelId = corpus().default_state("minecraft:bedrock").0.into();
    for (index, &section_y) in sections.iter().enumerate() {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let world_y = section_y * 16 + y;
                    let state = if world_y <= -60 { bedrock } else { stone };
                    column.set_in_section(index, x, y, z, state);
                }
            }
        }
    }
    (column, stone, bedrock)
}

fn ids() -> ModernCarverBlockIds {
    // No tag registry in a unit test, so the uncarvable set is supplied the
    // way the tag would: bedrock's states.
    ModernCarverBlockIds::for_test(vec![corpus().default_state("minecraft:bedrock").0.into()])
}

/// A tile of sources is filled in one pass over a strided volume; every
/// source in it has to run the carvers a single-point evaluation resolves to.
#[test]
fn the_tiled_sources_match_point_sampling() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::from_climate(Preset::Overworld.parameter_list(), carvers_of);
    let mut ws = Workspace::new();
    for (chunk_x, chunk_z) in [(0, 0), (-13, 7)] {
        for source_x in (chunk_x - 8)..=(chunk_x + 8) {
            for source_z in (chunk_z - 8)..=(chunk_z + 8) {
                let point = climate_target_at(&router, &mut ws, source_x * 4, 0, source_z * 4);
                let expected = table.carvers_at_for_test(point);
                let tiled = table.carvers_of_source_for_test(&router, &mut ws, source_x, source_z);
                assert!(
                    std::ptr::eq(expected.as_ptr(), tiled.as_ptr()),
                    "source ({source_x}, {source_z}) of chunk ({chunk_x}, {chunk_z})"
                );
            }
        }
    }
}

#[test]
fn carving_an_overworld_column_frees_space_and_spares_bedrock() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::from_climate(Preset::Overworld.parameter_list(), carvers_of);
    let block_ids = ids();
    let sections = y_sections();
    let mut ws = Workspace::new();

    let mut carved_columns = 0;
    for chunk_x in 0..3 {
        let (column, stone, bedrock) = stone_column();
        apply_modern_carvers(
            &column,
            chunk_x,
            0,
            12345,
            &router,
            &mut ws,
            &table,
            overworld_height(),
            &block_ids,
            &mut column_fluid_field(&router, chunk_x * 16, 0),
        );

        let mut oracle = column_fluid_field(&router, chunk_x * 16, 0);
        let mut oracle_ws = Workspace::new();
        let mut barrier = point_barrier(&router, &mut oracle_ws);
        let mut freed = 0;
        for (index, &section_y) in sections.iter().enumerate() {
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let world_y = section_y * 16 + y;
                        let state = column.get(x, world_y, z).unwrap();
                        if world_y <= -60 {
                            assert_eq!(state, bedrock, "bedrock carved at Y {world_y}");
                            continue;
                        }
                        if state == stone {
                            continue;
                        }
                        freed += 1;
                        let expected =
                            oracle.substance(chunk_x * 16 + x, world_y, z, 0.0, &mut barrier);
                        assert_eq!(
                            Some(state),
                            expected,
                            "wrong substance at Y {world_y} of section {index}"
                        );
                        assert!(
                            world_y <= -64 + 384 - 1 - 7,
                            "carved into the protected top at Y {world_y}"
                        );
                    }
                }
            }
        }
        if freed > 0 {
            carved_columns += 1;
        }
    }
    assert!(carved_columns > 0, "three chunks carved nothing at all");
}

/// The carvers every biome runs, built the way the freeze system builds them
/// from the same files.
fn carvers_by_biome() -> (
    &'static mcrs_minecraft_registry::Registry<Biome>,
    mcrs_minecraft_registry::Entries<Biome, Arc<[CarverConfig]>>,
) {
    let biomes =
        mcrs_minecraft_worldgen_testing::registry::<mcrs_minecraft_biome_file::BiomeFile>("biome");
    let configs: std::collections::BTreeMap<_, _> =
        mcrs_minecraft_worldgen_testing::registry::<CarverConfig>("carver")
            .into_iter()
            .collect();
    let registry = super::corpus_biomes();
    let lists = registry
        .iter()
        .map(|(_, name)| {
            let name = mcrs_minecraft_core::ResourceLocation::read(name.as_str())
                .expect("a corpus biome id");
            mcrs_minecraft_worldgen_testing::names_of(&biomes[&name].carvers)
                .iter()
                .map(|carver| {
                    configs
                        [&mcrs_minecraft_core::ResourceLocation::read(carver).expect("a carver id")]
                        .clone()
                })
                .collect()
        })
        .collect();
    let entries =
        mcrs_minecraft_registry::Entries::new(registry, lists).expect("one list for every biome");
    (registry, entries)
}

#[test]
fn the_freeze_resolution_builds_the_dimension_tables() {
    use crate::multi_noise_biomes::MultiNoiseBiomeTable;
    use mcrs_minecraft_biome::climate::{Parameter, ParameterPoint};

    let (biomes, carvers) = carvers_by_biome();
    let of_climate = |table: &MultiNoiseBiomeTable| {
        CarverBiomeTable::from_climate(table.climate(), |biome| {
            carvers.as_slice()[usize::from(biome)].clone()
        })
    };
    let of_entries =
        |entries| CarverBiomeTable::from_entries(entries, |biome| carvers[*biome].clone());

    let overworld_climate = MultiNoiseBiomeTable::of_preset(Preset::Overworld, biomes)
        .expect("every overworld preset biome is in the corpus");
    let overworld = of_climate(&overworld_climate);
    assert_eq!(overworld.entry_count(), 7594);

    let nether_climate = MultiNoiseBiomeTable::of_preset(Preset::Nether, biomes)
        .expect("every nether preset biome is in the corpus");
    let nether = of_climate(&nether_climate);
    assert_eq!(nether.entry_count(), 5);
    let wastes = nether.carvers_at_for_test(mcrs_minecraft_biome::climate::TargetPoint::new(
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    ));
    assert_eq!(wastes.len(), 1, "the nether wastes run one carver");
    assert!(matches!(wastes[0], CarverConfig::Cave { .. }));

    // A source that lists its biomes instead of naming a preset.
    let point = Parameter::point(0.0);
    let explicit = of_entries(vec![(
        ParameterPoint {
            temperature: point,
            humidity: point,
            continentalness: point,
            erosion: point,
            depth: point,
            weirdness: point,
            offset: 0,
        },
        biomes.by_name("minecraft:plains").expect("a corpus biome"),
    )])
    .expect("an explicit list resolves");
    assert_eq!(explicit.entry_count(), 1);

    // A biome with no carvers resolves to an empty list rather than to the
    // wrong one.
    let empty = of_entries(vec![(
        ParameterPoint {
            temperature: point,
            humidity: point,
            continentalness: point,
            erosion: point,
            depth: point,
            weirdness: point,
            offset: 0,
        },
        biomes.by_name("minecraft:the_end").expect("a corpus biome"),
    )])
    .unwrap();
    assert!(
        empty
            .carvers_at_for_test(mcrs_minecraft_biome::climate::TargetPoint::new(
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0
            ))
            .is_empty()
    );
}

/// A Beta source runs the carvers of the land biome the palette gives the
/// source chunk's first cell.
#[test]
fn a_beta_source_runs_the_carvers_of_its_palette_biome() {
    use bevy_math::IVec3;
    use mcrs_minecraft_biome::source::BiomeSource;
    use mcrs_minecraft_worldgen_density::router::{TEMPERATURE, VEGETATION};
    use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;

    let router = super::build_beta_router();
    let (source, _) = super::beta_surface::build_beta_biome_source();
    let BiomeSource::Beta { land_biomes, .. } = &source else {
        unreachable!("the helper builds a Beta source");
    };
    let cave: serde_json::Value =
        serde_json::from_slice(&std::fs::read(worldgen_dir().join("carver/cave.json")).unwrap())
            .unwrap();
    // A marker per land biome: a cave whose probability is the biome's index.
    let table = CarverBiomeTable::beta(&source, |biome| {
        let index = land_biomes.iter().position(|id| *id == biome).unwrap();
        let mut config = cave.clone();
        config["probability"] = serde_json::json!(index as f32 / 16.0);
        Arc::from([serde_json::from_value::<CarverConfig>(config).unwrap()])
    })
    .unwrap();

    let mut ws = Workspace::new();
    let mut seen = std::collections::BTreeSet::new();
    for source_x in (-64..64).step_by(7) {
        for source_z in (-64..64).step_by(5) {
            let volume = SampleGrid::new(
                IVec3::ONE,
                IVec3::new(source_x * 16, 0, source_z * 16),
                IVec3::ONE,
            );
            let mut climate = [0.0f32; 2];
            router.fill_roots(&mut ws, &volume, &[TEMPERATURE, VEGETATION], &mut climate);
            let biome = source.beta_biome(climate[0], climate[1]);
            let index = land_biomes.iter().position(|id| *id == biome).unwrap();
            let carvers = table.carvers_of_source_for_test(&router, &mut ws, source_x, source_z);
            assert_eq!(
                carvers[0].probability(),
                Some(index as f32 / 16.0),
                "source ({source_x}, {source_z})"
            );
            seen.insert(index);
        }
    }
    assert!(seen.len() > 1, "the sample crossed only one biome");
}
