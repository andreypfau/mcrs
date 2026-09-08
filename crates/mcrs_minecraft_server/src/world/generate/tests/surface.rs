use std::collections::{BTreeMap, HashMap};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_world::biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::material::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder, MaterialScratch, NO_WATER,
};
use mcrs_minecraft_worldgen::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_voxel_storage::VoxelId;

use crate::world::chunk::CancellationToken;
use crate::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::world::generate::surface::{Visit, descend_strip, set_block};
use crate::world::generate::{
    ColumnBlocks, NO_TOP, SurfaceIds, apply_material_surface, fill_column_dense_any,
    multi_noise_palettes, spans_dimension,
};

use super::{assets_root, corpus, density_function_registry, load_json_dir, noise_registry};

const AIR: VoxelId = VoxelId(0);
const STONE: VoxelId = VoxelId(1);
const WATER: VoxelId = VoxelId(2);

/// Every visit the descent made, as `(y, depth_above, depth_below, water)`.
fn walk(column: &ColumnBlocks, height: i32, min_y: i32) -> Vec<(i32, i32, i32, i32)> {
    let mut seen = Vec::new();
    descend_strip(column, 0, 0, height, min_y, WATER, |step| {
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

fn fill(column: &ColumnBlocks, range: std::ops::RangeInclusive<i32>, state: VoxelId) {
    for y in range {
        column.set(0, y, 0, state);
    }
}

#[test]
fn the_descent_reports_depths_water_and_the_floor_below_a_run() {
    let column = ColumnBlocks::new(&[0, 1]);
    fill(&column, 0..=9, STONE);
    fill(&column, 10..=12, WATER);

    let seen = walk(&column, 13, 0);

    // The first fluid block from above sets the water level one block over it,
    // and the look-ahead's read below the bottom ends the stone run at y = 0.
    let expected: Vec<_> = (0..=9)
        .rev()
        .enumerate()
        .map(|(step, y)| (y, step as i32 + 1, y + 1, 13))
        .collect();
    assert_eq!(seen, expected);
}

#[test]
fn an_all_stone_column_ends_its_run_at_the_bottom_of_the_dimension() {
    let column = ColumnBlocks::new(&[0, 1]);
    fill(&column, 0..=31, STONE);

    let seen = walk(&column, 32, 0);

    assert_eq!(seen.first().copied(), Some((31, 1, 32, NO_WATER)));
    // Treating the read below the bottom as "stop" instead of "not solid"
    // leaves this at a sentinel some thirty thousand blocks deep, and every
    // ceiling rule fires.
    assert_eq!(seen.last().copied(), Some((0, 32, 1, NO_WATER)));
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
    assert_eq!(seen.first().copied(), Some((47, 1, 48, NO_WATER)));
    assert!(
        seen.iter().any(|(y, _, _, _)| *y == 15),
        "the descent stopped at the gap instead of skipping it"
    );
    assert_eq!(seen.last().copied(), Some((0, 32, 1, NO_WATER)));
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

/// The preset's biomes numbered as the palette numbers them. Any biome a rule
/// names that the preset does not is given one shared id no grid cell can hold,
/// so its sets fold to `never` exactly as they should.
const ABSENT_BIOME: u32 = 250;

pub(super) fn biome_ids() -> HashMap<String, u32> {
    let mut ids = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = ids.len() as u32;
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    ids
}

pub(super) fn overworld_material_router(seed: u64, ids: &HashMap<String, u32>) -> NoiseRouter {
    let path = assets_root().join("noise_settings/overworld.json");
    let settings: NoiseGeneratorSettings =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = load_json_dir("material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        load_json_dir("material_condition");
    let inputs = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block: &|state| {
            corpus()
                .block(state.name.as_str())
                .map(|b| b.default_state_id.into())
        },
        biome: &|id| Some(ids.get(id.as_str()).copied().unwrap_or(ABSENT_BIOME)),
    };
    build_router(
        &settings,
        &density_function_registry(),
        &noise_registry(),
        seed,
        corpus().default_state("minecraft:stone").into(),
        corpus().default_state("minecraft:water").into(),
        Some(&inputs),
    )
    .expect("the overworld material rule compiles")
}

fn surface_ids(ids: &HashMap<String, u32>) -> SurfaceIds {
    let biome = |name: &str| ids.get(name).copied().unwrap_or(ABSENT_BIOME);
    SurfaceIds {
        eroded_badlands: biome("minecraft:eroded_badlands"),
        frozen_ocean: biome("minecraft:frozen_ocean"),
        deep_frozen_ocean: biome("minecraft:deep_frozen_ocean"),
        snow_block: corpus().default_state("minecraft:snow_block").into(),
        packed_ice: corpus().default_state("minecraft:packed_ice").into(),
    }
}

/// Fill one column and run the surface pass over it, returning the blocks.
pub(super) fn surfaced_column(
    router: &NoiseRouter,
    ids: &HashMap<String, u32>,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    bypass_shortcuts: bool,
) -> ColumnBlocks {
    let table = MultiNoiseBiomeTable::resolve(
        &MultiNoiseBiomeSource {
            preset: Some(ResourceLocation::parse("minecraft:overworld").unwrap()),
            biomes: None,
        },
        |biome| u8::try_from(ids[biome]).ok(),
    )
    .expect("the overworld preset resolves");

    let mut column = ColumnBlocks::new(y_sections);
    let mut filled = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        y_sections,
        router,
        None,
        None,
        &CancellationToken::new(),
    )
    .expect("the column fills");
    let (_, grid) =
        multi_noise_palettes(router, &table, section_x * 16, section_z * 16, y_sections);

    let mut scratch = MaterialScratch::default();
    scratch.bypass_shortcuts(bypass_shortcuts);
    apply_material_surface(
        &column,
        section_x,
        section_z,
        &mut filled.tops,
        &grid.expect("the multi-noise fill widens a grid"),
        router,
        &surface_ids(ids),
        &mut scratch,
    );
    column
}

/// The descent enters each strip one block above its highest non-air block, and
/// that height is read off the sections the dispatch carries. A slice would be
/// surfaced as if its cut were the sky, so the dispatch site leaves it alone.
#[test]
fn only_a_whole_column_is_surfaced() {
    let ids = biome_ids();
    let router = overworld_material_router(2, &ids);
    let whole: Vec<i32> = (-4..20).collect();

    assert_eq!(router.noise_min_y(), -64);
    assert_eq!(router.noise_height(), 384);
    assert!(spans_dimension(&whole, &router));
    assert!(!spans_dimension(&whole[..12], &router));
    assert!(!spans_dimension(&whole[12..], &router));
    assert!(!spans_dimension(&[], &router));

    let mut holed = whole.clone();
    holed.remove(6);
    holed.push(20);
    assert!(!spans_dimension(&holed, &router));
}

/// The rules turn a stone column into a soil profile. Without them every strip
/// ends in the default block, which is what this would show as a failure.
#[test]
fn an_overworld_column_gets_grass_over_dirt_over_stone() {
    let ids = biome_ids();
    let router = overworld_material_router(2, &ids);
    let y_sections: Vec<i32> = (-4..8).collect();
    let column = surfaced_column(&router, &ids, 3, -7, &y_sections, false);

    let grass = VoxelId::from(corpus().default_state("minecraft:grass_block"));
    let dirt = VoxelId::from(corpus().default_state("minecraft:dirt"));
    let stone = router.default_block_state();

    let mut grassed = 0;
    let mut over_stone = 0;
    for x in 0..16 {
        for z in 0..16 {
            let Some(top) = (-64..128).rev().find(|&y| {
                column
                    .get(x, y, z)
                    .is_some_and(|state| state != AIR && state != router.default_fluid_state())
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

/// A condition given the wrong scope produces plausible terrain and passes
/// every other test; a run with every cache forced to miss does not. The second
/// column is in sulfur caves, whose bands are the corpus's only `is_3d` noise
/// threshold, so the noise cache's scope is covered as well as the condition
/// cache's.
#[test]
fn bypassing_every_shortcut_writes_the_same_blocks() {
    let ids = biome_ids();
    let router = overworld_material_router(2, &ids);
    let sulfur = VoxelId::from(corpus().default_state("minecraft:sulfur"));
    let cinnabar = VoxelId::from(corpus().default_state("minecraft:cinnabar"));
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
        let memoised = surfaced_column(&router, &ids, section_x, section_z, &y_sections, false);
        let bypassed = surfaced_column(&router, &ids, section_x, section_z, &y_sections, true);

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
    ids: &HashMap<String, u32>,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
) -> usize {
    let table = MultiNoiseBiomeTable::resolve(
        &MultiNoiseBiomeSource {
            preset: Some(ResourceLocation::parse("minecraft:overworld").unwrap()),
            biomes: None,
        },
        |biome| u8::try_from(ids[biome]).ok(),
    )
    .expect("the overworld preset resolves");
    let (_, grid) =
        multi_noise_palettes(router, &table, section_x * 16, section_z * 16, y_sections);
    let mut present = [false; 256];
    for id in &grid.expect("the multi-noise fill widens a grid").ids {
        present[*id as usize] = true;
    }
    present.iter().filter(|seen| **seen).count()
}

/// The preset's biomes as a registry, and the ids it gave them.
fn overworld_biome_registry() -> (
    mcrs_minecraft_core::RegistrySnapshot<Biome>,
    HashMap<String, u32>,
) {
    let mut assets = bevy_asset::Assets::<Biome>::default();
    let mut names: Vec<String> = Vec::new();
    for (_, biome) in overworld_parameter_list().values() {
        if !names.iter().any(|seen| seen == biome) {
            names.push((*biome).to_owned());
        }
    }
    let handles: Vec<_> = names
        .iter()
        .map(|_| assets.add(super::beta_biome_palette::make_beta_biome()))
        .collect();
    let pairs: Vec<_> = names
        .iter()
        .zip(&handles)
        .map(|(name, handle)| {
            (
                ResourceLocation::parse(name).expect("a preset biome name"),
                handle.id(),
            )
        })
        .collect();
    let snapshot = mcrs_minecraft_core::RegistrySnapshot::<Biome>::build(pairs, &assets, |_| {
        Ok(mcrs_minecraft_nbt::compound::NbtCompound::new())
    });
    let ids = names
        .iter()
        .map(|name| {
            let id = snapshot.by_location(name).expect("the registry holds it");
            (name.clone(), id)
        })
        .collect();
    (snapshot, ids)
}

/// A dispatch owes only the sections it carries — the ticket layer caps how many
/// it spawns a tick and cuts a column's sections across two of them — and the
/// bedrock floor is a material rule, so a dispatch that skipped the rules over
/// its slice would leave the world open at the bottom.
#[test]
fn a_dispatch_carrying_part_of_a_column_still_lays_its_bedrock_floor() {
    use bevy_app::{App, Update};
    use bevy_ecs::entity::Entity;
    use bevy_tasks::{TaskPoolBuilder, block_on};
    use mcrs_minecraft_protocol::ColumnPos;
    use mcrs_minecraft_world::biome::source::BiomeSource;
    use mcrs_minecraft_world::worldgen::beta_biome::ActiveBiomeSource;
    use mcrs_minecraft_worldgen::bevy::DimensionNoiseRouter;
    use std::sync::Arc;

    use super::blocks;
    use crate::world::chunk::{
        CHUNK_TASK_POOL, ColumnKey, ColumnScheduler, PendingColumn, dispatch_column_generation,
    };

    let (registry, ids) = overworld_biome_registry();
    let router = overworld_material_router(2, &ids);
    CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());

    let mut app = App::new();
    app.insert_resource(DimensionNoiseRouter(Arc::new(router)));
    app.insert_resource(blocks().clone());
    app.insert_resource(registry);
    app.insert_resource(ActiveBiomeSource(Arc::new(BiomeSource::MultiNoise(
        MultiNoiseBiomeSource {
            preset: Some(ResourceLocation::parse("minecraft:overworld").unwrap()),
            biomes: None,
        },
    ))));

    let col = ColumnPos::new(3, -7);
    let carried = [-4, 3, 4];
    let sections: Vec<(Entity, i32)> = carried
        .iter()
        .map(|&y| (app.world_mut().spawn_empty().id(), y))
        .collect();
    let key = ColumnKey::new(0, col);
    let mut scheduler = ColumnScheduler::default();
    scheduler.priority_index.insert(col, key);
    scheduler.pending.insert(key, PendingColumn::new(sections));
    app.insert_resource(scheduler);
    app.add_systems(Update, dispatch_column_generation);
    app.update();

    let in_flight = app
        .world_mut()
        .resource_mut::<ColumnScheduler>()
        .in_flight
        .pop()
        .expect("the column was dispatched");
    let result = block_on(in_flight.task);

    assert_eq!(
        result.sections.len(),
        carried.len(),
        "the dispatch returned sections it was not asked for"
    );
    let bedrock = VoxelId::from(corpus().default_state("minecraft:bedrock"));
    let floor = result
        .sections
        .iter()
        .find(|(_, pos, _)| pos.y == carried[0])
        .expect("the bottom section came back");
    let (palette, _) = floor.2.as_ref().expect("the bottom section carries blocks");
    let mut states = Vec::with_capacity(ColumnBlocks::SECTION_VOLUME);
    palette.0.for_each(|state| states.push(state));
    // The palette runs y, then z, then x, so the first layer is the floor of
    // the dimension, which `bedrock_floor` covers whole.
    assert!(
        states[..256].iter().all(|state| *state == bedrock),
        "the bottom of the world is open: the material rules never ran over this dispatch"
    );
}
