use super::{corpus, router_blocks};
use crate::modern_carvers::{ModernCarverBlockIds, TerrainCarving};
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::surface::{Visit, descend_strip, set_block};
use crate::task::CancellationToken;
use crate::{
    ColumnBlocks, NO_TOP, SurfaceIds, SurfaceStates, apply_material_surface, fill_column_dense_any,
    multi_noise_grid, multi_noise_palettes, spans_dimension,
};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_registry::{Registry, RegistrySet};
use mcrs_minecraft_worldgen_carver::mask::CarvingMask;
use mcrs_minecraft_worldgen_density::aquifer::WAY_BELOW_MIN_Y;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen_surface::compile::{MaterialProgram, build_router_and_material};
use mcrs_minecraft_worldgen_surface::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder, MaterialScratch, NO_WATER,
};
use mcrs_minecraft_worldgen_testing::{registry, registry_in};
use std::collections::{BTreeMap, HashMap};

const AIR: VoxelId = VoxelId(0);

const STONE: VoxelId = VoxelId(1);

const WATER: VoxelId = VoxelId(2);

/// Every visit the descent made, as `(y, depth_above, depth_below, water)`.
fn walk(column: &ColumnBlocks, height: i32, min_y: i32) -> Vec<(i32, i32, i32, i32)> {
    let mut seen = Vec::new();
    descend_strip(column, 0, 0, height, min_y, [WATER, VoxelId(3)], |step| {
        if let Visit::Block {
            y,
            depth_above,
            depth_below,
            water_level,
        } = step
        {
            seen.push((y, depth_above, depth_below, water_level));
        }
    });
    seen
}

pub fn fill(column: &ColumnBlocks, range: std::ops::RangeInclusive<i32>, state: VoxelId) {
    for y in range {
        column.set(0, y, 0, state);
    }
}

#[test]
fn the_descent_reports_depths_water_and_the_floor_below_a_run() {
    let column = ColumnBlocks::new(&[0, 1]);
    fill(&column, 3..=9, STONE);
    fill(&column, 10..=12, WATER);

    let seen = walk(&column, 13, 0);

    // The first fluid block from above sets the water level one block over it,
    // and the air under y = 3 is the floor the depth below counts from.
    let expected: Vec<_> = (3..=9)
        .rev()
        .enumerate()
        .map(|(step, y)| (y, step as i32 + 1, y - 2, 13))
        .collect();
    assert_eq!(seen, expected);
}

#[test]
fn a_run_down_to_the_bottom_of_the_dimension_has_no_floor_under_it() {
    let column = ColumnBlocks::new(&[0, 1]);
    fill(&column, 0..=31, STONE);

    let seen = walk(&column, 32, 0);

    let below = |y: i32| y - WAY_BELOW_MIN_Y + 1;
    assert_eq!(seen.first().copied(), Some((31, 1, below(31), NO_WATER)));
    assert_eq!(seen.last().copied(), Some((0, 32, below(0), NO_WATER)));
}

#[test]
fn a_section_this_dispatch_does_not_carry_is_skipped_rather_than_the_bottom() {
    let column = ColumnBlocks::new(&[0, 2]);
    fill(&column, 0..=15, STONE);
    fill(&column, 32..=47, STONE);

    let seen = walk(&column, 48, 0);

    // The look-ahead skips the gap too, so the run below y = 47 reaches the
    // floor of the dimension rather than the floor of the dispatch: treating the
    // gap as a ceiling would make this 16 and fire every ceiling rule at y = 32.
    let below = |y: i32| y - WAY_BELOW_MIN_Y + 1;
    assert_eq!(seen.first().copied(), Some((47, 1, below(47), NO_WATER)));
    assert!(
        seen.iter().any(|(y, _, _, _)| *y == 15),
        "the descent stopped at the gap instead of skipping it"
    );
    assert_eq!(seen.last().copied(), Some((0, 32, below(0), NO_WATER)));
}

/// A rule that carves the top block away lowers the strip's height, which the
/// gradients of the strips after it read.
#[test]
fn writing_air_at_the_top_lowers_the_height_and_writing_a_block_raises_it() {
    let column = ColumnBlocks::new(&[0, 1]);
    fill(&column, 0..=9, STONE);
    fill(&column, 12..=12, STONE);
    let mut tops = [NO_TOP; 256];
    tops[0] = 12;

    set_block(&column, &mut tops, 0, 0, 12, 0, AIR);
    assert_eq!(tops[0], 9);
    set_block(&column, &mut tops, 0, 0, 5, 0, AIR);
    assert_eq!(tops[0], 9, "carving below the top leaves the height alone");
    set_block(&column, &mut tops, 0, 0, 20, 0, STONE);
    assert_eq!(tops[0], 20);

    for y in (0..=9).rev() {
        set_block(&column, &mut tops, 0, 0, y, 0, AIR);
    }
    set_block(&column, &mut tops, 0, 0, 20, 0, AIR);
    assert_eq!(tops[0], NO_TOP);
}

/// The preset's biomes numbered as the palette numbers them.
pub(super) fn biome_ids() -> HashMap<String, u16> {
    let mut ids = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = u16::try_from(ids.len()).unwrap();
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    ids
}

/// The corpus read with the biomes numbered as `ids` does and every biome it
/// leaves out after them, which no cell of the preset's grid holds.
fn corpus_over(ids: &HashMap<String, u16>) -> RegistrySet {
    super::registries_over(&registry_of(ids))
}

/// A registry numbering the biomes as `ids` does.
fn registry_of(ids: &HashMap<String, u16>) -> Registry<Biome> {
    let mut named: Vec<(u16, &str)> = ids.iter().map(|(name, id)| (*id, name.as_str())).collect();
    named.sort();
    let names: Vec<&str> = named.into_iter().map(|(_, name)| name).collect();
    super::ordered_biome_registry(&names)
}

/// The overworld preset's climate table over the biomes `ids` numbers.
fn table_over(ids: &HashMap<String, u16>) -> MultiNoiseBiomeTable {
    MultiNoiseBiomeTable::resolve(
        &MultiNoiseBiomeSource {
            preset: Some(crate::tests::parameter_list_id("minecraft:overworld")),
            biomes: None,
        },
        &registry_of(ids),
        &crate::tests::parameter_lists().1,
    )
    .expect("the overworld preset resolves")
}

pub fn overworld_material_router(
    seed: u64,
    ids: &HashMap<String, u16>,
) -> (NoiseRouter, MaterialProgram) {
    let settings: NoiseGeneratorSettings = mcrs_minecraft_worldgen_testing::read(
        "noise_settings",
        &rl!("minecraft:overworld").to_arc(),
    );
    let set = corpus_over(ids);
    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = registry_in(&set, "material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        registry_in(&set, "material_condition");
    let biome_tags = set.tags().expect("the corpus holds the biome tags");
    let inputs = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block: &|state| {
            corpus()
                .block(state.name.as_str())
                .map(|b| b.default_state_id.0.into())
        },
        biome_tags: &biome_tags,
    };
    build_router_and_material(
        &settings,
        &registry("density_function"),
        &registry("noise"),
        seed,
        router_blocks(corpus()),
        &inputs,
    )
    .expect("the overworld material rule compiles")
}

fn surface_ids(
    ids: &HashMap<String, u16>,
) -> mcrs_minecraft_registry::shared::Resolved<SurfaceIds> {
    super::surface_ids_over(&registry_of(ids))
}

fn surface_states() -> SurfaceStates {
    SurfaceStates::new(&super::blocks().0)
}

/// Fill one column and run the surface pass over it, returning the blocks.
pub(super) fn surfaced_column(
    router: &NoiseRouter,
    material: &MaterialProgram,
    ids: &HashMap<String, u16>,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    bypass_shortcuts: bool,
) -> ColumnBlocks {
    let table = table_over(ids);

    let mut column = ColumnBlocks::new(y_sections);
    let mut filled = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        y_sections,
        router,
        None,
        None,
        None,
        &CancellationToken::new(),
    )
    .expect("the column fills");
    let biomes = multi_noise_palettes(router, &table, section_x * 16, section_z * 16, y_sections);

    let mut scratch = MaterialScratch::default();
    scratch.bypass_shortcuts(bypass_shortcuts);
    apply_material_surface(
        &column,
        section_x,
        section_z,
        &mut filled.tops,
        &biomes,
        y_sections[0],
        router,
        material,
        &surface_ids(ids),
        &surface_states(),
        &mut scratch,
        None,
    );
    column
}

/// The descent enters each strip one block above its highest non-air block, and
/// that height is read off the sections the dispatch carries. A slice would be
/// surfaced as if its cut were the sky, so the dispatch site leaves it alone.
#[test]
fn only_a_whole_column_is_surfaced() {
    let (min_y, height) = (-64, 384);
    let spans = |sections: &[i32]| spans_dimension(sections, min_y, height);
    let whole: Vec<i32> = (-4..20).collect();

    assert!(spans(&whole));
    assert!(!spans(&whole[..12]));
    assert!(!spans(&whole[12..]));
    assert!(!spans(&[]));

    let mut holed = whole.clone();
    holed.remove(6);
    holed.push(20);
    assert!(!spans(&holed));
}

/// The rules turn a stone column into a soil profile. Without them every strip
/// ends in the default block, which is what this would show as a failure.
#[test]
fn an_overworld_column_gets_grass_over_dirt_over_stone() {
    let ids = biome_ids();
    let (router, material) = overworld_material_router(2, &ids);
    let y_sections: Vec<i32> = (-4..8).collect();
    let column = surfaced_column(&router, &material, &ids, 3, -7, &y_sections, false);

    let grass = VoxelId::from(corpus().default_state("minecraft:grass_block").0);
    let dirt = VoxelId::from(corpus().default_state("minecraft:dirt").0);
    let stone = router.default_block_state;

    let mut grassed = 0;
    let mut over_stone = 0;
    for x in 0..16 {
        for z in 0..16 {
            let Some(top) = (-64..128).rev().find(|&y| {
                column
                    .get(x, y, z)
                    .is_some_and(|state| state != AIR && state != router.default_fluid_state)
            }) else {
                continue;
            };
            if column.get(x, top, z) != Some(grass) {
                continue;
            }
            assert_eq!(
                column.get(x, top - 1, z),
                Some(dirt),
                "under grass at {x},{z}"
            );
            grassed += 1;
            let soil = (1..)
                .take_while(|below| column.get(x, top - below, z) == Some(dirt))
                .count() as i32;
            if column.get(x, top - soil - 1, z) == Some(stone) {
                over_stone += 1;
            }
        }
    }
    assert!(grassed > 0, "not one strip of the column ended in grass");
    assert!(over_stone > 0, "grass and dirt never sat on stone");
}

/// Carving is decided block by block inside the descent: a carved grass top
/// opens to air and the dirt it bared is surfaced again as the new top, while
/// the fill's own water is never carved.
#[test]
fn a_carved_top_bares_dirt_that_is_surfaced_again_and_water_is_never_carved() {
    let ids = biome_ids();
    let (router, material) = overworld_material_router(2, &ids);
    let y_sections: Vec<i32> = (-4..20).collect();
    let (mut regrown, mut kept_water) = (0, 0);
    for (section_x, section_z) in [(3, -7), (40, 0), (-12, 5), (0, 0)] {
        let plain = surfaced_column(
            &router,
            &material,
            &ids,
            section_x,
            section_z,
            &y_sections,
            false,
        );

        let grass = VoxelId::from(corpus().default_state("minecraft:grass_block").0);
        let dirt = VoxelId::from(corpus().default_state("minecraft:dirt").0);
        let fluid = router.default_fluid_state;
        let top_of = |column: &ColumnBlocks, x: i32, z: i32| {
            (-64..320)
                .rev()
                .find(|&y| column.get(x, y, z).is_some_and(|state| state != AIR))
        };

        let mut mask = CarvingMask::new(-63, 319 - 7);
        let mut grass_tops = Vec::new();
        let mut water_tops = Vec::new();
        for x in 0..16 {
            for z in 0..16 {
                let Some(top) = top_of(&plain, x, z) else {
                    continue;
                };
                match plain.get(x, top, z) {
                    Some(state) if state == grass && plain.get(x, top - 1, z) == Some(dirt) => {
                        grass_tops.push((x, top, z));
                        mask.carve(x, top, z);
                    }
                    Some(state) if state == fluid => {
                        water_tops.push((x, top, z));
                        mask.carve(x, top, z);
                    }
                    _ => {}
                }
            }
        }

        let table = table_over(&ids);
        let mut column = ColumnBlocks::new(&y_sections);
        let mut filled = fill_column_dense_any(
            &mut column,
            section_x,
            section_z,
            &y_sections,
            &router,
            None,
            None,
            None,
            &CancellationToken::new(),
        )
        .expect("the column fills");
        let biomes =
            multi_noise_palettes(&router, &table, section_x * 16, section_z * 16, &y_sections);
        let carver_ids = ModernCarverBlockIds::for_test(Vec::new());
        apply_material_surface(
            &column,
            section_x,
            section_z,
            &mut filled.tops,
            &biomes,
            y_sections[0],
            &router,
            &material,
            &surface_ids(&ids),
            &surface_states(),
            &mut MaterialScratch::default(),
            Some(&mut TerrainCarving::new(
                &mask,
                &carver_ids,
                &mut filled.fluid,
                &router,
                section_x,
                section_z,
            )),
        );

        for &(x, top, z) in &grass_tops {
            let opened = column.get(x, top, z).unwrap();
            assert_ne!(
                opened, grass,
                "the carved top at {x},{top},{z} still stands"
            );
            if opened == AIR {
                assert_eq!(
                    column.get(x, top - 1, z),
                    Some(grass),
                    "the dirt bared at {x},{},{z} was not surfaced again",
                    top - 1
                );
                regrown += 1;
            }
        }
        for &(x, y, z) in &water_tops {
            assert_eq!(
                column.get(x, y, z),
                Some(fluid),
                "water carved at {x},{y},{z}"
            );
            kept_water += 1;
        }
    }
    assert!(regrown > 0, "no carved top opened to air");
    assert!(kept_water > 0, "no carved water was left standing");
}

/// A condition given the wrong scope produces plausible terrain and passes
/// every other test; a run with every cache forced to miss does not. The second
/// column is in sulfur caves, whose bands are the corpus's only `is_3d` noise
/// threshold, so the noise cache's scope is covered as well as the condition
/// cache's.
#[test]
fn bypassing_every_shortcut_writes_the_same_blocks() {
    let ids = biome_ids();
    let (router, material) = overworld_material_router(2, &ids);
    let sulfur = VoxelId::from(corpus().default_state("minecraft:sulfur").0);
    let cinnabar = VoxelId::from(corpus().default_state("minecraft:cinnabar").0);
    let mut banded = 0;
    let mut multi_biome = 0;

    for (section_x, section_z, y_sections) in [
        (3, -7, (2..6).collect::<Vec<i32>>()),
        (-22, 38, (-4..0).collect()),
        (3, -7, (-4..20).collect()),
        (-22, 38, (-4..20).collect()),
    ] {
        if grid_biomes(&router, &ids, section_x, section_z, &y_sections) > 1 {
            multi_biome += 1;
        }
        let memoised = surfaced_column(
            &router,
            &material,
            &ids,
            section_x,
            section_z,
            &y_sections,
            false,
        );
        let bypassed = surfaced_column(
            &router,
            &material,
            &ids,
            section_x,
            section_z,
            &y_sections,
            true,
        );

        for (index, &section_y) in y_sections.iter().enumerate() {
            let (left, right) = (memoised.section_cells(index), bypassed.section_cells(index));
            for (at, (left, right)) in left.iter().zip(right).enumerate() {
                let (left, right) = (left.get(), right.get());
                assert_eq!(
                    left, right,
                    "section {section_y} block {at} of {section_x},{section_z} differs with the shortcuts bypassed"
                );
                if left == sulfur || left == cinnabar {
                    banded += 1;
                }
            }
        }
    }
    assert!(
        banded > 0,
        "no sulfur cave band was painted, so the 3D noise cache went untested"
    );
    assert!(
        multi_biome > 0,
        "every column held one biome, so folding a biome set over a strip went untested"
    );
}

/// How many distinct biomes a column's widened grid holds. One means the fold
/// over the whole column already settles every biome condition, and the
/// per-strip fold never runs.
fn grid_biomes(
    router: &NoiseRouter,
    ids: &HashMap<String, u16>,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
) -> usize {
    let table = table_over(ids);
    let grid = multi_noise_grid(router, &table, section_x * 16, section_z * 16, y_sections)
        .expect("the multi-noise fill widens a grid");
    let mut present = [false; 256];
    for id in &grid.ids {
        present[*id as usize] = true;
    }
    present.iter().filter(|seen| **seen).count()
}

/// The preset's biomes as a registry, and the ids it gave them.
pub fn overworld_biome_registry() -> (Registry<Biome>, HashMap<String, u16>) {
    let mut names: Vec<&str> = Vec::new();
    for (_, biome) in overworld_parameter_list().values() {
        if !names.contains(biome) {
            names.push(biome);
        }
    }
    names.sort_unstable();
    let registry = super::biome_registry(&names);
    let ids = names
        .iter()
        .map(|name| {
            let id = registry.by_name(name).expect("the registry holds it");
            ((*name).to_owned(), id.number())
        })
        .collect();
    (registry, ids)
}

/// The per-dimension inputs a column stage reads, resolved the way
/// `dispatch_column_generation` resolves them for the pool.
pub fn fill_context(
    router: NoiseRouter,
    material: MaterialProgram,
    registry: Registry<Biome>,
    source: mcrs_minecraft_biome::source::BiomeSource,
) -> crate::stages::FillContext {
    use mcrs_minecraft_biome::source::BiomeSource;

    use super::blocks;
    let surface = (
        super::surface_ids_over(&registry),
        SurfaceStates::new(&blocks().0),
    );
    let multi_noise = match &source {
        BiomeSource::MultiNoise(multi) => Some(std::sync::Arc::new(
            MultiNoiseBiomeTable::resolve(multi, &registry, &crate::tests::parameter_lists().1)
                .expect("the source resolves a table"),
        )),
        _ => None,
    };
    crate::stages::FillContext {
        biome: Some((std::sync::Arc::new(source), registry)),
        program: crate::stages::ColumnProgram {
            generator: crate::stages::ColumnGenerator::Modern {
                multi_noise,
                surface: Some(surface),
                carver_blocks: std::sync::Arc::new(
                    crate::modern_carvers::ModernCarverBlockIds::for_test(Vec::new()),
                ),
            },
            carvers: None,
            features: None,
        },
        material: Some(std::sync::Arc::new(material)),
        ..super::bare_fill_context(router)
    }
}

/// A `minecraft:fixed` source answers one biome at every quart cell, so the
/// rules that biome selects run over whatever terrain the noise gives. Badlands
/// is the case worth pinning: it is the sole user of the clay band table, and
/// no multi-noise sample any test takes reaches it.
fn a_fixed_biome_source_drives_that_biome_s_material_rules() {
    use mcrs_minecraft_biome::source::BiomeSource;
    use mcrs_minecraft_protocol::ColumnPos;

    use crate::stages::fill_column;
    use crate::task::CancellationToken;

    let generate = |biome: &str| -> (Vec<VoxelId>, Vec<u8>) {
        let (registry, ids) = overworld_biome_registry();
        let (router, material) = overworld_material_router(2, &ids);
        let fixed = registry.require_by_name(biome).expect("a biome name");
        let ctx = fill_context(
            router,
            material,
            registry,
            BiomeSource::Fixed { biome: fixed },
        );

        let y_sections = ctx.y_sections.clone();
        let mut column = ColumnBlocks::new(&y_sections);
        let snapshot = fill_column(
            &ctx,
            ColumnPos::new(3, -7),
            &mut column,
            &CancellationToken::new(),
        )
        .expect("the fill was not cancelled");

        let mut states = Vec::new();
        let mut biomes = Vec::new();
        for payload in &snapshot.sections {
            let Some((palette, biome_palette)) = payload.as_ref() else {
                continue;
            };
            palette.0.for_each(|state| states.push(state));
            for cx in 0..16 {
                for cy in 0..16 {
                    for cz in 0..16 {
                        biomes.push(biome_palette.get_cell(cx, cy, cz));
                    }
                }
            }
        }
        (states, biomes)
    };

    let badlands_id = {
        let (registry, _) = overworld_biome_registry();
        registry
            .by_name("minecraft:badlands")
            .expect("the overworld preset names badlands")
            .narrow::<u8>()
            .expect("a biome id the palette can store")
    };

    let (badlands_blocks, badlands_biomes) = generate("minecraft:badlands");
    assert!(
        !badlands_biomes.is_empty() && badlands_biomes.iter().all(|id| *id == badlands_id),
        "a fixed source must answer one biome at every cell"
    );

    let terracotta = VoxelId::from(corpus().default_state("minecraft:terracotta").0);
    let orange = VoxelId::from(corpus().default_state("minecraft:orange_terracotta").0);
    assert!(
        badlands_blocks
            .iter()
            .any(|state| *state == terracotta || *state == orange),
        "the clay band rule never ran: no terracotta anywhere in the column"
    );

    let (plains_blocks, _) = generate("minecraft:plains");
    assert_ne!(
        badlands_blocks, plains_blocks,
        "the fixed biome did not reach the rule dispatch: two biomes gave one column"
    );
}

/// The same column under constant biome containers, with the shortcuts on and off.
fn surfaced_column_fixed(
    router: &NoiseRouter,
    material: &MaterialProgram,
    ids: &HashMap<String, u16>,
    biome: &str,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    bypass_shortcuts: bool,
) -> ColumnBlocks {
    use mcrs_minecraft_level::palette::BiomePalette;

    let mut column = ColumnBlocks::new(y_sections);
    let mut filled = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        y_sections,
        router,
        None,
        None,
        None,
        &CancellationToken::new(),
    )
    .expect("the column fills");

    let id = u8::try_from(ids[biome]).expect("a biome id the palette can store");
    let biomes = vec![BiomePalette::homogeneous(id); y_sections.len()];

    let mut scratch = MaterialScratch::default();
    scratch.bypass_shortcuts(bypass_shortcuts);
    apply_material_surface(
        &column,
        section_x,
        section_z,
        &mut filled.tops,
        &biomes,
        y_sections[0],
        router,
        material,
        &surface_ids(ids),
        &surface_states(),
        &mut scratch,
        None,
    );
    column
}

/// A run that reaches the clay band rule settles, and the band it writes varies
/// with y inside that run. Nothing else in the corpus reaches that rule, so
/// without a badlands column the settled path is never walked.
#[test]
fn a_fixed_badlands_source_runs_the_clay_bands_and_a_settled_run_writes_what_the_descent_would() {
    a_fixed_biome_source_drives_that_biome_s_material_rules();
    settled_badlands_runs_write_the_bands_the_descent_would(&[(3, -7)]);
}

mod exhaustive {
    #[test]
    fn a_settled_badlands_run_writes_the_bands_the_descent_would() {
        super::settled_badlands_runs_write_the_bands_the_descent_would(&[(3, -7), (-22, 38)]);
    }
}

fn settled_badlands_runs_write_the_bands_the_descent_would(columns: &[(i32, i32)]) {
    let ids = biome_ids();
    let (router, material) = overworld_material_router(2, &ids);
    let y_sections: Vec<i32> = (-4..20).collect();
    let bands: Vec<VoxelId> = material.clay_bands().to_vec();
    assert!(!bands.is_empty());

    let mut banded = 0;
    for biome in [
        "minecraft:badlands",
        "minecraft:eroded_badlands",
        "minecraft:wooded_badlands",
    ] {
        for &(section_x, section_z) in columns {
            let settled = surfaced_column_fixed(
                &router,
                &material,
                &ids,
                biome,
                section_x,
                section_z,
                &y_sections,
                false,
            );
            let walked = surfaced_column_fixed(
                &router,
                &material,
                &ids,
                biome,
                section_x,
                section_z,
                &y_sections,
                true,
            );

            for index in 0..y_sections.len() {
                let (left, right) = (settled.section_cells(index), walked.section_cells(index));
                for (at, (left, right)) in left.iter().zip(right).enumerate() {
                    let (left, right) = (left.get(), right.get());
                    assert_eq!(
                        left, right,
                        "{biome} block {at} of section {index} at {section_x},{section_z} \
                         differs between the settled run and the full descent"
                    );
                    if bands.contains(&left) {
                        banded += 1;
                    }
                }
            }
        }
    }
    assert!(
        banded > 0,
        "no clay band was written, so the settled bandlands path went untested"
    );
}
