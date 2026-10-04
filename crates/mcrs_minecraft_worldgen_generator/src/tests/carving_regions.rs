use std::collections::HashMap;
use std::sync::Arc;

use mcrs_minecraft_biome::climate::ParameterPoint;
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::value_provider::{FloatProvider, HeightContext};
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_carver::mask::CarvingMask;
use mcrs_minecraft_worldgen_carver::modern::{SOURCE_RADIUS, carve_source_into};
use mcrs_minecraft_worldgen_carver::target::Region;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;

use super::beta_surface::build_beta_biome_source;
use super::modern_carvers::carvers_of;
use super::surface::{fill_context, overworld_biome_registry, overworld_material_router};
use super::{build_settings_router, corpus};
use crate::ColumnBlocks;
use crate::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, REGION_CAPACITY, REGION_WIDTH, carve_sources,
    carving_mask, modern_carving_mask, whole_climate_space,
};
use crate::stages::{ColumnGenerator, FillContext, extent, fill_column};
use crate::task::CancellationToken;

fn table(preset: &str, width: i32, capacity: usize) -> CarverBiomeTable {
    CarverBiomeTable::resolve(preset, carvers_of)
        .expect("a known preset")
        .with_region(width, capacity)
}

fn per_column(
    router: &NoiseRouter,
    ws: &mut Workspace,
    table: &CarverBiomeTable,
    seed: u64,
    height: HeightContext,
    (x, z): (i32, i32),
) -> CarvingMask {
    carve_sources(
        || unreachable!("no Beta carver runs in a modern dimension"),
        x,
        z,
        seed as i64,
        router,
        ws,
        table,
        height,
    )
}

fn square(x: i32, z: i32, width: i32) -> Vec<(i32, i32)> {
    (x..x + width)
        .flat_map(|x| (z..z + width).map(move |z| (x, z)))
        .collect()
}

fn assert_regions_match_columns(
    settings: &str,
    preset: &str,
    seed: u64,
    width: i32,
    columns: &[(i32, i32)],
) -> usize {
    let router = build_settings_router(settings, seed);
    let height = extent(&router);
    let regional = table(preset, width, 8);
    let alone = table(preset, 1, 0);
    let mut ws = Workspace::new();
    let mut carved = 0;
    for &(x, z) in columns {
        let slot = modern_carving_mask(x, z, seed as i64, &router, &mut ws, &regional, height);
        let expected = per_column(&router, &mut ws, &alone, seed, height, (x, z));
        assert!(
            *slot == expected,
            "column ({x}, {z}) of a width {width} region, {settings} seed {seed}, differs from the \
             column carved on its own"
        );
        carved += usize::from(!expected.is_empty());
    }
    carved
}

#[test]
fn a_region_slot_equals_the_per_column_sources() {
    let land = assert_regions_match_columns(
        "overworld",
        "minecraft:overworld",
        12345,
        4,
        &square(8, -8, 4),
    );
    let ocean = assert_regions_match_columns(
        "overworld",
        "minecraft:overworld",
        845,
        4,
        &square(-4, -4, 4),
    );
    let nether =
        assert_regions_match_columns("nether", "minecraft:nether", 12345, 4, &square(-4, 0, 4));
    assert!(
        land > 0 && ocean > 0 && nether > 0,
        "a region carved nothing, so nothing was compared: land {land}, ocean {ocean}, nether {nether}"
    );
}

#[test]
fn a_column_below_zero_belongs_to_the_region_below_zero() {
    let seed = 12345;
    let router = build_settings_router("overworld", seed);
    let height = extent(&router);
    let regional = table("minecraft:overworld", 3, 4);
    let alone = table("minecraft:overworld", 1, 0);
    let mut ws = Workspace::new();

    let mut carved = 0;
    let mut ask = |x: i32, z: i32| {
        let slot = modern_carving_mask(x, z, seed as i64, &router, &mut ws, &regional, height);
        let expected = per_column(&router, &mut Workspace::new(), &alone, seed, height, (x, z));
        assert!(*slot == expected, "column ({x}, {z})");
        carved += usize::from(!expected.is_empty());
        slot
    };

    let below = ask(-1, 0);
    let first_below = ask(-3, 0);
    let above = ask(0, 0);
    let last_above = ask(2, 2);
    let beside = ask(0, -1);

    assert!(below.same_region_as(&first_below));
    assert!(above.same_region_as(&last_above));
    assert!(!below.same_region_as(&above));
    assert!(!above.same_region_as(&beside));
    assert!(carved > 0, "every column compared was empty");
}

fn long_canyon_table(width: i32, capacity: usize) -> CarverBiomeTable {
    let mut canyon: CarverConfig =
        mcrs_minecraft_worldgen_testing::read("carver", &ResourceLocation::minecraft("canyon"));
    let CarverConfig::Canyon {
        probability, shape, ..
    } = &mut canyon
    else {
        panic!("the shipped canyon is a canyon");
    };
    *probability = 1.0;
    shape.distance_factor = FloatProvider::Constant(5.0);
    let entries: Vec<(ParameterPoint, String)> =
        vec![(whole_climate_space(), "minecraft:plains".to_owned())];
    CarverBiomeTable::from_entries(entries, move |_| Arc::from([canyon.clone()]))
        .expect("one biome")
        .with_region(width, capacity)
}

#[test]
fn a_source_past_a_columns_reach_does_not_carve_it_through_a_shared_region() {
    let seed = 12345;
    let router = build_settings_router("overworld", seed);
    let height = extent(&router);
    let regional = long_canyon_table(4, 4);
    let alone = long_canyon_table(1, 0);
    let mut ws = Workspace::new();

    let mut carved = 0;
    for (x, z) in square(0, 0, 4) {
        let slot = modern_carving_mask(x, z, seed as i64, &router, &mut ws, &regional, height);
        let expected = per_column(&router, &mut ws, &alone, seed, height, (x, z));
        assert!(
            *slot == expected,
            "column ({x}, {z}) was carved by a source outside its own source radius"
        );
        carved += usize::from(!expected.is_empty());
    }
    assert!(carved > 0, "the canyons carved nothing");
}

#[test]
fn a_region_is_built_once_for_all_its_columns() {
    let seed = 12345;
    let router = build_settings_router("overworld", seed);
    let height = extent(&router);
    let shared = table("minecraft:overworld", 4, 4);
    let columns = square(0, 0, 4);

    let slots: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|worker| {
                let (router, shared, columns) = (&router, &shared, &columns);
                scope.spawn(move || {
                    let mut ws = Workspace::new();
                    let mut order = columns.clone();
                    order.rotate_left(worker * 2);
                    order
                        .into_iter()
                        .map(|(x, z)| {
                            (
                                (x, z),
                                modern_carving_mask(
                                    x,
                                    z,
                                    seed as i64,
                                    router,
                                    &mut ws,
                                    shared,
                                    height,
                                ),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a worker finished"))
            .collect()
    });

    assert_eq!(shared.region_builds_for_test(), 1);
    assert_eq!(shared.region_entries_for_test(), Some(1));
    let (_, first) = &slots[0];
    assert!(slots.iter().all(|(_, slot)| slot.same_region_as(first)));
    let alone = table("minecraft:overworld", 1, 0);
    let mut ws = Workspace::new();
    for (column, slot) in &slots {
        let expected = per_column(&router, &mut ws, &alone, seed, height, *column);
        assert!(**slot == expected, "column {column:?}");
    }
}

#[test]
fn the_region_cache_holds_at_most_its_capacity() {
    let seed = 12345;
    let router = build_settings_router("overworld", seed);
    let height = extent(&router);
    let kept = table("minecraft:overworld", 2, 3);
    let mut ws = Workspace::new();
    let mut ask = |region: i32| {
        modern_carving_mask(region * 2, 0, seed as i64, &router, &mut ws, &kept, height);
    };

    ask(0);
    ask(1);
    ask(2);
    assert_eq!(kept.region_entries_for_test(), Some(3));
    assert_eq!(kept.region_builds_for_test(), 3);
    assert_eq!(kept.region_bytes_bound(height), 3 * 2 * 2 * 12_032);

    ask(0);
    assert_eq!(
        kept.region_builds_for_test(),
        3,
        "a held region is not rebuilt"
    );

    ask(3);
    assert_eq!(kept.region_entries_for_test(), Some(3));
    assert_eq!(kept.region_builds_for_test(), 4);

    ask(0);
    ask(2);
    ask(3);
    assert_eq!(
        kept.region_builds_for_test(),
        4,
        "the regions asked for most recently are the ones held"
    );
    ask(1);
    assert_eq!(
        kept.region_builds_for_test(),
        5,
        "the region asked for longest ago was the one replaced"
    );

    for region in 4..40 {
        ask(region);
        assert_eq!(kept.region_entries_for_test(), Some(3));
    }
}

#[test]
fn two_seeds_do_not_share_a_region() {
    let shared = table("minecraft:overworld", 2, 4);
    let alone = table("minecraft:overworld", 1, 0);
    let mut ws = Workspace::new();
    let mut differing = 0;
    for seed in [12345, 845] {
        let router = build_settings_router("overworld", seed);
        let height = extent(&router);
        for (x, z) in square(0, 0, 2) {
            let slot = modern_carving_mask(x, z, seed as i64, &router, &mut ws, &shared, height);
            let expected = per_column(&router, &mut ws, &alone, seed, height, (x, z));
            assert!(*slot == expected, "seed {seed} column ({x}, {z})");
        }
    }
    for (x, z) in square(0, 0, 2) {
        let masks: Vec<_> = [12345, 845]
            .map(|seed| {
                let router = build_settings_router("overworld", seed);
                let height = extent(&router);
                per_column(&router, &mut ws, &alone, seed, height, (x, z))
            })
            .into();
        differing += usize::from(masks[0] != masks[1]);
    }
    assert!(differing > 0, "the two seeds carve the same columns alike");
    assert_eq!(shared.region_builds_for_test(), 2);
    assert_eq!(shared.region_entries_for_test(), Some(2));
}

#[test]
fn two_heights_do_not_share_a_region() {
    let seed = 12345;
    let router = build_settings_router("overworld", seed);
    let tall = extent(&router);
    let short = HeightContext {
        min_y: 0,
        depth: 256,
        sea_level: 63,
    };
    let shared = table("minecraft:overworld", 2, 4);
    let alone = table("minecraft:overworld", 1, 0);
    let mut ws = Workspace::new();

    for height in [tall, short, tall] {
        for (x, z) in square(0, 0, 2) {
            let slot = modern_carving_mask(x, z, seed as i64, &router, &mut ws, &shared, height);
            let expected = per_column(&router, &mut ws, &alone, seed, height, (x, z));
            assert_eq!(slot.min_y(), expected.min_y());
            assert_eq!(slot.max_y(), expected.max_y());
            assert!(*slot == expected, "{height:?} column ({x}, {z})");
        }
    }
    assert_eq!(shared.region_builds_for_test(), 2);
    assert_eq!(shared.region_entries_for_test(), Some(2));
}

#[test]
fn a_region_of_one_column_is_not_kept() {
    let seed = 12345;
    let router = build_settings_router("overworld", seed);
    let height = extent(&router);
    let single = table("minecraft:overworld", 1, 16);
    let mut ws = Workspace::new();
    for (x, z) in square(-2, -2, 4) {
        let slot = modern_carving_mask(x, z, seed as i64, &router, &mut ws, &single, height);
        let expected = per_column(&router, &mut ws, &single, seed, height, (x, z));
        assert!(*slot == expected, "column ({x}, {z})");
    }
    assert_eq!(single.region_entries_for_test(), None);
}

#[test]
fn the_beta_generator_has_no_region_cache() {
    let (source, _) = build_beta_biome_source();
    let beta = super::beta_carver_table(&source);
    assert_eq!(beta.region_entries_for_test(), None);
}

#[test]
#[should_panic(expected = "a Beta table carves one column at a time")]
fn a_beta_table_refuses_a_region() {
    let (source, _) = build_beta_biome_source();
    super::beta_carver_table(&source).with_region(4, 8);
}

fn overworld_context(seed: u64) -> FillContext {
    let (registry, ids) = overworld_biome_registry();
    let (router, material) = overworld_material_router(seed, &ids);
    let source = BiomeSource::MultiNoise(MultiNoiseBiomeSource {
        preset: Some(ResourceLocation::parse("minecraft:overworld").expect("a preset")),
        biomes: None,
    });
    let mut context = fill_context(router, material, registry, source);
    let bedrock: VoxelId = corpus().default_state("minecraft:bedrock").into();
    if let ColumnGenerator::Modern { carver_blocks, .. } = &mut context.program.generator {
        *carver_blocks = Arc::new(ModernCarverBlockIds::for_test(vec![bedrock]));
    }
    context
}

fn fill(context: &FillContext, (x, z): (i32, i32)) -> (ColumnBlocks, Vec<u16>) {
    let mut buffer = ColumnBlocks::new(&context.y_sections);
    let snapshot = fill_column(
        context,
        ColumnPos::new(x, z),
        &mut buffer,
        &CancellationToken::new(),
    )
    .expect("the fill was not cancelled");
    let mut blocks = Vec::new();
    for section in &snapshot.sections {
        match section {
            Some((palette, _)) => palette.0.for_each(|state| blocks.push(state.0)),
            None => blocks.push(u16::MAX),
        }
    }
    (buffer, blocks)
}

fn blocks_of(context: &FillContext, column: (i32, i32)) -> Vec<u16> {
    fill(context, column).1
}

fn generate(
    context: &FillContext,
    order: &[(i32, i32)],
    threads: usize,
) -> HashMap<(i32, i32), Vec<u16>> {
    let chunk = order.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let workers: Vec<_> = order
            .chunks(chunk)
            .map(|share| {
                scope.spawn(move || {
                    share
                        .iter()
                        .map(|&column| (column, blocks_of(context, column)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a worker finished"))
            .collect()
    })
}

#[test]
fn a_columns_blocks_do_not_depend_on_the_region_the_order_or_the_cache() {
    let columns = square(-4, -2, 4)
        .into_iter()
        .chain(square(4, 4, 3))
        .collect::<Vec<_>>();
    let mut reversed = columns.clone();
    reversed.reverse();
    let strided: Vec<_> = (0..columns.len())
        .map(|index| columns[(index * 7) % columns.len()])
        .collect();
    assert_eq!(
        strided
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        columns.len(),
        "the stride visits each column once"
    );

    for (seed, surfaced) in [(845, true), (12345, true), (845, false)] {
        let mut context = overworld_context(seed);
        if !surfaced {
            context.material = None;
        }
        let with = |context: &mut FillContext, width, capacity| {
            context.program.carvers = Some(Arc::new(table("minecraft:overworld", width, capacity)));
        };

        with(&mut context, 1, 0);
        let expected = generate(&context, &columns, 1);
        assert!(
            expected.values().any(|blocks| blocks.contains(&0)),
            "the baseline generated no air at all"
        );

        for (width, capacity, order, threads) in [
            (4, 16, &columns, 1),
            (4, 2, &reversed, 1),
            (3, 1, &strided, 1),
            (8, 3, &strided, 1),
            (4, 16, &columns, 4),
            (3, 2, &reversed, 4),
        ] {
            with(&mut context, width, capacity);
            let got = generate(&context, order, threads);
            for column in &columns {
                assert!(
                    got[column] == expected[column],
                    "seed {seed} surfaced {surfaced}: column {column:?} differs at width {width}, \
                     capacity {capacity}, {threads} thread(s)"
                );
            }
        }
    }
}

type Pair = ((i32, i32), (i32, i32));

fn wet_border_pairs(
    context: &FillContext,
    filled: &HashMap<(i32, i32), ColumnBlocks>,
) -> Vec<Pair> {
    let router = context.router.as_ref();
    let height = extent(router);
    let table = context
        .program
        .carvers
        .as_ref()
        .expect("the context carries a carver table");
    let water = router.water_state;
    let mut ws = Workspace::new();
    let masks: HashMap<_, _> = filled
        .keys()
        .map(|&(x, z)| {
            let mask = modern_carving_mask(
                x,
                z,
                router.world_seed as i64,
                router,
                &mut ws,
                table,
                height,
            );
            ((x, z), mask)
        })
        .collect();

    let wet = |at: (i32, i32), x: i32, y: i32, z: i32| {
        masks[&at].contains(x, y, z)
            && !masks[&at].contains(x, y + 1, z)
            && filled[&at].get(x, y + 1, z) == Some(water)
    };
    let mut pairs = Vec::new();
    for &(x, z) in filled.keys() {
        let low = (x, z);
        for (high, along_x) in [((x + 1, z), true), ((x, z + 1), false)] {
            if !filled.contains_key(&high) {
                continue;
            }
            let touches = (masks[&low].min_y()..=masks[&low].max_y()).any(|y| {
                (0..16).any(|along| {
                    let (low_at, high_at) = if along_x {
                        ((15, along), (0, along))
                    } else {
                        ((along, 15), (along, 0))
                    };
                    wet(low, low_at.0, y, low_at.1) && wet(high, high_at.0, y, high_at.1)
                })
            });
            if touches {
                pairs.push((low, high));
            }
        }
    }
    pairs.sort_unstable();
    pairs
}

fn crosses_regions(width: i32, (low, high): Pair) -> bool {
    low.0.div_euclid(width) != high.0.div_euclid(width)
        || low.1.div_euclid(width) != high.1.div_euclid(width)
}

fn assert_blocks_independent_of_width(
    seed: u64,
    origin: i32,
    side: i32,
    widths: &[i32],
    threads: usize,
    at_least: usize,
) {
    let columns = square(origin, origin, side);
    let mut context = overworld_context(seed);
    context.program.carvers = Some(Arc::new(table("minecraft:overworld", 1, 0)));

    let (buffers, expected): (HashMap<_, _>, HashMap<_, _>) = columns
        .iter()
        .map(|&column| {
            let (buffer, blocks) = fill(&context, column);
            ((column, buffer), (column, blocks))
        })
        .unzip();
    let wet = wet_border_pairs(&context, &buffers);

    for &width in widths {
        let crossing = wet
            .iter()
            .filter(|pair| crosses_regions(width, **pair))
            .count();
        assert!(
            crossing >= at_least,
            "seed {seed}, columns {origin}..{}: only {crossing} column borders with carved cells \
             under water on both sides lie between two regions of width {width}, {at_least} \
             are needed for the comparison to mean anything",
            origin + side
        );

        let regions_touched = (side / width + 2) as usize;
        context.program.carvers = Some(Arc::new(table(
            "minecraft:overworld",
            width,
            regions_touched * regions_touched,
        )));

        let router = context.router.as_ref();
        let regional = context.program.carvers.as_ref().expect("a carver table");
        let alone = table("minecraft:overworld", 1, 0);
        let height = extent(router);
        let mut ws = Workspace::new();
        for column in wet
            .iter()
            .filter(|pair| crosses_regions(width, **pair))
            .flat_map(|&(low, high)| [low, high])
        {
            let slot = modern_carving_mask(
                column.0,
                column.1,
                router.world_seed as i64,
                router,
                &mut ws,
                regional,
                height,
            );
            let alone = per_column(router, &mut ws, &alone, router.world_seed, height, column);
            assert!(
                *slot == alone,
                "seed {seed}: the mask of column {column:?} at region width {width} differs from \
                 the column carved on its own"
            );
        }

        let got = generate(&context, &columns, threads);
        for column in &columns {
            assert!(
                got[column] == expected[column],
                "seed {seed}: column {column:?} differs at region width {width}"
            );
        }
    }
}

#[test]
fn a_columns_blocks_do_not_depend_on_the_region_width() {
    assert_blocks_independent_of_width(845, -4, 8, &[2, 4, 8], 1, 3);
}

const CAVE: usize = 0;
const CANYON: usize = 1;

type RegionCounts = [[u32; 3]; 2];

/// Per carver kind: the walks the region driver makes, the walks of sources
/// that start, and the walks that mark a cell in any column of the region.
fn region_counts(settings: &str, preset: &str, seed: u64, columns: i32) -> RegionCounts {
    let router = build_settings_router(settings, seed);
    let height = extent(&router);
    let regional = table(preset, REGION_WIDTH, REGION_CAPACITY);
    let width = REGION_WIDTH;
    assert_eq!(columns % width, 0, "the block is made of whole regions");
    let mut ws = Workspace::new();
    let mut counts = RegionCounts::default();
    let fresh =
        || -> Vec<CarvingMask> { (0..width * width).map(|_| carving_mask(height)).collect() };

    for origin_x in (-columns / 2..columns / 2).step_by(width as usize) {
        for origin_z in (-columns / 2..columns / 2).step_by(width as usize) {
            let mut walked = fresh();
            let mut region = Region::new(origin_x, origin_z, width, &mut walked);
            for source_x in origin_x - SOURCE_RADIUS..=origin_x + width - 1 + SOURCE_RADIUS {
                for source_z in origin_z - SOURCE_RADIUS..=origin_z + width - 1 + SOURCE_RADIUS {
                    let carvers =
                        regional.carvers_of_source_for_test(&router, &mut ws, source_x, source_z);
                    for (index, config) in carvers.iter().enumerate() {
                        let kind = match config {
                            CarverConfig::Cave { .. } => CAVE,
                            CarverConfig::Canyon { .. } => CANYON,
                            CarverConfig::BetaCave => unreachable!("a modern dimension"),
                        };
                        counts[kind][0] += 1;
                        let walk = |region: &mut Region<'_>| {
                            carve_source_into(
                                config,
                                index,
                                seed as i64,
                                height,
                                region,
                                source_x,
                                source_z,
                            )
                        };
                        if !walk(&mut region) {
                            continue;
                        }
                        counts[kind][1] += 1;

                        let mut alone = fresh();
                        walk(&mut Region::new(origin_x, origin_z, width, &mut alone));
                        counts[kind][2] += u32::from(alone.iter().any(|mask| !mask.is_empty()));
                    }
                }
            }
            for (slot, mask) in walked.iter().enumerate() {
                let (x, z) = (
                    origin_x + slot as i32 / width,
                    origin_z + slot as i32 % width,
                );
                let driven =
                    modern_carving_mask(x, z, seed as i64, &router, &mut ws, &regional, height);
                assert!(
                    *driven == *mask,
                    "the walks counted for region ({origin_x}, {origin_z}) are not the ones the driver makes: column ({x}, {z}) differs"
                );
            }
        }
    }
    counts
}

const REGION_SOURCE_COUNTS: [RegionCounts; 3] = [
    [[10368, 1182, 118], [5184, 76, 7]],
    [[10368, 1334, 64], [5184, 120, 15]],
    [[5184, 1204, 87], [0, 0, 0]],
];

#[test]
fn the_source_counts_per_region_are_what_they_were_measured() {
    let got = [
        (
            "overworld seed 12345",
            region_counts("overworld", "minecraft:overworld", 12345, 8),
        ),
        (
            "overworld seed 845",
            region_counts("overworld", "minecraft:overworld", 845, 8),
        ),
        (
            "nether seed 12345",
            region_counts("nether", "minecraft:nether", 12345, 8),
        ),
    ];
    let moved: Vec<String> = got
        .iter()
        .zip(&REGION_SOURCE_COUNTS)
        .filter(|((_, got), pinned)| got != *pinned)
        .map(|((label, got), pinned)| format!("{label}: pinned {pinned:?}, now {got:?}"))
        .collect();
    assert!(moved.is_empty(), "{}", moved.join("\n"));
}

mod exhaustive {
    use super::*;

    #[test]
    fn a_region_slot_equals_the_per_column_sources_at_every_width_and_origin() {
        let mut carved = 0;
        for seed in [12345, 845, 7] {
            for width in [2, 3, 4, 5, 8] {
                for origin in [-width, -1, 0] {
                    carved += assert_regions_match_columns(
                        "overworld",
                        "minecraft:overworld",
                        seed,
                        width,
                        &square(origin, 3 - origin, width),
                    );
                }
            }
        }
        for width in [3, 4] {
            carved += assert_regions_match_columns(
                "nether",
                "minecraft:nether",
                12345,
                width,
                &square(-width, -1, width),
            );
        }
        assert!(carved > 0);
    }

    fn blocks_independent_of_width_at_both_origins(seed: u64) {
        for origin in [-8, -5] {
            assert_blocks_independent_of_width(seed, origin, 16, &[2, 3, 4, 5, 8], 4, 5);
        }
    }

    #[test]
    fn a_columns_blocks_do_not_depend_on_the_region_width_for_seed_845() {
        blocks_independent_of_width_at_both_origins(845);
    }

    #[test]
    fn a_columns_blocks_do_not_depend_on_the_region_width_for_seed_12345() {
        blocks_independent_of_width_at_both_origins(12345);
    }

    #[test]
    fn a_columns_blocks_do_not_depend_on_the_region_width_for_seed_7() {
        blocks_independent_of_width_at_both_origins(7);
    }

    #[test]
    fn a_columns_blocks_do_not_depend_on_the_region_width_for_seed_1() {
        blocks_independent_of_width_at_both_origins(1);
    }
}
