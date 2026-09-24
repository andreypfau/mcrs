use crate::modern_carvers::TerrainCarving;
use crate::multi_noise_biomes::BiomeGrid;
use crate::{ColumnBlocks, NO_TOP};
use bevy_math::IVec3;
use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::zoom::{FiddleCache, obfuscate_seed, quart_cell, uniform_corners};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, QuartPos};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_worldgen_density::aquifer::WAY_BELOW_MIN_Y;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;
use mcrs_minecraft_worldgen_surface::{
    MaterialEval, MaterialScratch, NO_WATER, SettledState, SurfaceNoise,
};
use std::cell::RefCell;

/// The blocks and biomes the two hardcoded landforms name, which no rule does.
pub struct SurfaceIds {
    pub eroded_badlands: u32,
    pub frozen_ocean: u32,
    pub deep_frozen_ocean: u32,
    pub snow_block: VoxelId,
    pub packed_ice: VoxelId,
    pub dirt: VoxelId,
}

impl SurfaceIds {
    pub fn resolve(blocks: &BlockDefinitions, biomes: &RegistrySnapshot<Biome>) -> Self {
        let biome = |name: &str| {
            biomes.by_location(name).unwrap_or_else(|| {
                panic!(
                    "the surface stage names the biome `{name}`, which the registry does not hold"
                )
            })
        };
        Self {
            eroded_badlands: biome("minecraft:eroded_badlands"),
            frozen_ocean: biome("minecraft:frozen_ocean"),
            deep_frozen_ocean: biome("minecraft:deep_frozen_ocean"),
            snow_block: blocks.default_state("minecraft:snow_block").into(),
            packed_ice: blocks.default_state("minecraft:packed_ice").into(),
            dirt: blocks.default_state("minecraft:dirt").into(),
        }
    }
}

/// Whether a column buffer covers the dimension rather than a slice of it.
///
/// The descent starts one block above each strip's highest non-air block, and
/// that height is read off the sections the buffer holds. Given a slice, the cut
/// is what the descent enters at, with no depth above it — so the top of the
/// slice is surfaced as if it were the sky, and a sheet of grass over dirt lands
/// in the middle of the rock. The biome grid and the ore-vein prefill region are
/// cut the same way, and the rules that lay the bedrock floor are never reached.
/// Sections past the noise range are the End's, which the dimension carries and
/// the noise does not fill.
pub fn spans_dimension(y_sections: &[i32], router: &NoiseRouter) -> bool {
    let bottom = router.noise.min_y >> 4;
    let top = bottom + (router.noise.height as i32 >> 4);
    let contiguous = y_sections
        .iter()
        .enumerate()
        .all(|(index, &section_y)| section_y == y_sections[0] + index as i32);
    contiguous
        && y_sections.first().is_some_and(|&first| first <= bottom)
        && y_sections.last().is_some_and(|&last| last + 1 >= top)
}

/// Rewrite every strip of a filled column from the top down, carving it as it
/// goes where `carving` is given.
///
/// The surface gradients read the tops the fill left, so a pillar or an
/// iceberg changes neither its neighbours' steepness nor their gradient rules;
/// a strip's own descent still starts above its pillar.
#[allow(clippy::too_many_arguments)]
pub fn apply_material_surface(
    column: &ColumnBlocks,
    section_x: i32,
    section_z: i32,
    tops: &mut [i32; 256],
    grid: &BiomeGrid,
    router: &NoiseRouter,
    material: &MaterialProgram,
    ids: &SurfaceIds,
    scratch: &mut MaterialScratch,
    carving: Option<&mut TerrainCarving<'_, '_>>,
) {
    thread_local! {
        static FIDDLE: RefCell<FiddleCache> = RefCell::new(FiddleCache::default());
    }
    FIDDLE.with_borrow_mut(|fiddle| {
        apply_material_surface_with(
            column, section_x, section_z, tops, grid, router, material, ids, scratch, carving,
            fiddle,
        )
    });
}

fn apply_material_surface_with(
    column: &ColumnBlocks,
    section_x: i32,
    section_z: i32,
    tops: &mut [i32; 256],
    grid: &BiomeGrid,
    router: &NoiseRouter,
    program: &MaterialProgram,
    ids: &SurfaceIds,
    scratch: &mut MaterialScratch,
    mut carving: Option<&mut TerrainCarving<'_, '_>>,
    fiddle: &mut FiddleCache,
) {
    let block_x = section_x * 16;
    let block_z = section_z * 16;
    let min_y = router.noise.min_y;
    let sea_level = router.sea_level;
    let stone = router.default_block_state;
    let fluid = router.default_fluid_state;
    let lava = router.lava_state;
    let zoom_seed = obfuscate_seed(router.world_seed as i64);

    // The fold runs over the whole grid, border ring included: the zoom can
    // select a cell in the ring, so a biome present only there can still win
    // for a block inside the column.
    let mut present = [false; 256];
    for id in &grid.ids {
        present[*id as usize] = true;
    }
    let biomes: Vec<u32> = (0..256u32).filter(|id| present[*id as usize]).collect();

    let top = tops.iter().copied().max().unwrap_or(NO_TOP).max(min_y);
    // Every corner the eight-way pick can reach from a block of this column:
    // the parent cell of the lowest block, and one past that of the highest.
    fiddle.begin(
        zoom_seed,
        [(block_x - 2) >> 2, (min_y - 2) >> 2, (block_z - 2) >> 2],
        [6, ((top + 1 - min_y) >> 2) + 3, 6],
    );
    let mut eval = MaterialEval::new(
        router,
        program,
        scratch,
        |x, y, z| {
            let pos = BlockPos::new(x, y, z);
            uniform_corners(pos, |quart| grid_biome(grid, quart))
                .unwrap_or_else(|| grid_biome(grid, fiddle.quart_cell(pos)))
        },
        |bx, bz, lo, hi, out| reachable_biomes(grid, bx, bz, lo, hi, out),
        block_x,
        block_z,
        top,
        &biomes,
    );

    let fill_tops = *tops;
    let mut settled = Vec::new();
    for x in 0..16 {
        for z in 0..16 {
            let (bx, bz) = (block_x + x, block_z + z);
            let starting_height = height_of(tops, x, z, min_y) + 1;
            let surface_biome = zoom_biome(grid, zoom_seed, bx, starting_height, bz);
            if surface_biome == ids.eroded_badlands {
                eroded_badlands(
                    column,
                    tops,
                    program,
                    x,
                    z,
                    bx,
                    bz,
                    starting_height,
                    min_y,
                    stone,
                    fluid,
                );
            }

            let height = height_of(tops, x, z, min_y) + 1;
            let gradient_x = height_of(&fill_tops, (x + 1).min(15), z, min_y)
                - height_of(&fill_tops, (x - 1).max(0), z, min_y);
            let gradient_z = height_of(&fill_tops, x, (z + 1).min(15), min_y)
                - height_of(&fill_tops, x, (z - 1).max(0), min_y);
            eval.begin_strip(bx, bz, gradient_x, gradient_z);
            let mut run = 0;
            let mut carved_top = false;
            descend_strip(
                column,
                x,
                z,
                height,
                min_y,
                [fluid, lava],
                |step| match step {
                    Visit::Run {
                        top,
                        bottom,
                        ceiling,
                        depth_above,
                        water_level,
                    } => {
                        eval.settled_runs(
                            top,
                            bottom,
                            ceiling,
                            depth_above,
                            water_level,
                            &mut settled,
                        );
                        run = 0;
                    }
                    Visit::Open { y } => {
                        carved_top &= carving
                            .as_deref()
                            .is_some_and(|carving| carving.is_carved(x, y, z));
                    }
                    Visit::Block {
                        y,
                        depth_above,
                        depth_below,
                        water_level,
                    } => {
                        while run < settled.len() && y < settled[run].0 {
                            run += 1;
                        }
                        let mut state = if run < settled.len() && y <= settled[run].1 {
                            match settled[run].2 {
                                SettledState::Block(state) => state,
                                SettledState::Bandlands => Some(eval.bandlands_at(y)),
                            }
                        } else {
                            eval.update_y(depth_above, depth_below, water_level, y);
                            eval.apply()
                        };
                        if let Some(substance) = carving
                            .as_deref_mut()
                            .and_then(|carving| carving.substance(x, y, z, state))
                        {
                            carved_top |= depth_above == 1;
                            set_block(column, tops, min_y, x, y, z, substance);
                            return;
                        }
                        // Carving the top of a run away bares the block under
                        // it, which is surfaced again as if it were the top.
                        if carved_top {
                            if state == Some(ids.dirt) {
                                eval.update_y(1, depth_below, water_level, y);
                                state = eval.apply();
                            }
                            carved_top = false;
                        }
                        match state {
                            Some(state) if state == stone => {}
                            state => set_block(column, tops, min_y, x, y, z, state.unwrap_or_default()),
                        }
                    }
                },
            );

            if surface_biome == ids.frozen_ocean || surface_biome == ids.deep_frozen_ocean {
                frozen_ocean(
                    column,
                    tops,
                    program,
                    eval.min_surface_level(),
                    min_y,
                    x,
                    z,
                    bx,
                    bz,
                    starting_height,
                    sea_level,
                    fluid,
                    ids,
                    carving.as_deref(),
                );
            }
        }
    }
}

/// What the descent hands its visitor: each solid run as it is entered, with
/// the depth its top block has — fluid above does not reset it — then every
/// solid block of it, and every air or fluid block between runs.
pub(crate) enum Visit {
    Run {
        top: i32,
        bottom: i32,
        ceiling: i32,
        depth_above: i32,
        water_level: i32,
    },
    Block {
        y: i32,
        depth_above: i32,
        depth_below: i32,
        water_level: i32,
    },
    Open {
        y: i32,
    },
}

/// Walk one strip from `height` down to the bottom of the dimension, handing
/// every solid block its two depths and the water level above it, and each
/// solid run its top, its bottom, the ceiling its depth below counts from and
/// the water level over it as it is entered. Any of `fluids` counts as fluid,
/// as the reference reads any non-empty fluid state: lava from the field's
/// floor is not rock to surface.
///
/// A position this dispatch does not carry is skipped rather than ending the
/// descent. A run that reaches the floor has no ceiling under it, so its depth
/// below counts from far beneath the world and no ceiling rule fires there.
pub(crate) fn descend_strip(
    column: &ColumnBlocks,
    x: i32,
    z: i32,
    height: i32,
    min_y: i32,
    fluids: [VoxelId; 2],
    mut visit: impl FnMut(Visit),
) {
    let air = VoxelId::default();
    let is_fluid = |state: VoxelId| fluids.contains(&state);
    let mut depth_above = 0;
    let mut water_level = NO_WATER;
    let mut next_ceiling = i32::MAX;

    for y in (min_y..=height).rev() {
        let Some(old) = column.get(x, y, z) else {
            continue;
        };
        if old == air {
            depth_above = 0;
            water_level = NO_WATER;
            visit(Visit::Open { y });
        } else if is_fluid(old) {
            if water_level == NO_WATER {
                water_level = y + 1;
            }
            visit(Visit::Open { y });
        } else {
            if next_ceiling >= y {
                next_ceiling = (min_y..y)
                    .rev()
                    .find(|&look| {
                        column
                            .get(x, look, z)
                            .is_some_and(|old| old == air || is_fluid(old))
                    })
                    .map_or(WAY_BELOW_MIN_Y, |floor| floor + 1);
                visit(Visit::Run {
                    top: y,
                    bottom: next_ceiling.max(min_y),
                    ceiling: next_ceiling,
                    depth_above: depth_above + 1,
                    water_level,
                });
            }
            depth_above += 1;
            visit(Visit::Block {
                y,
                depth_above,
                depth_below: y - next_ceiling + 1,
                water_level,
            });
        }
    }
}

fn height_of(tops: &[i32; 256], x: i32, z: i32, min_y: i32) -> i32 {
    match tops[(z * 16 + x) as usize] {
        NO_TOP => min_y - 1,
        top => top,
    }
}

/// The biome the fiddled zoom selects, read out of the widened grid rather than
/// out of a neighbouring column's stored palette.
fn zoom_biome(grid: &BiomeGrid, zoom_seed: i64, x: i32, y: i32, z: i32) -> u32 {
    let pos = BlockPos::new(x, y, z);
    uniform_corners(pos, |quart| grid_biome(grid, quart))
        .unwrap_or_else(|| grid_biome(grid, quart_cell(zoom_seed, pos)))
}

/// Every biome the zoom can select for the strip at `(bx, bz)` anywhere in
/// `lo..=hi`. The pick reaches the two parent cells around the block in x and
/// z and, across the range, every cell from the lowest block's parent to one
/// past the highest block's, so scanning those four grid columns over that
/// span covers it. A superset is sound: the caller only folds it.
fn reachable_biomes(
    grid: &BiomeGrid,
    bx: i32,
    bz: i32,
    lo: i32,
    hi: i32,
    out: &mut Vec<u32>,
) -> bool {
    let min = grid.volume.min_block();
    let size = grid.volume.size();
    let cell = |value: i32, origin: i32, limit: i32| (value - origin).clamp(0, limit - 1);
    let first = cell(((lo - 2) >> 2) - (min.y >> 2), 0, size.y);
    let last = cell(((hi - 2) >> 2) + 1 - (min.y >> 2), 0, size.y);
    let parent_x = ((bx - 2) >> 2) - (min.x >> 2);
    let parent_z = ((bz - 2) >> 2) - (min.z >> 2);

    out.clear();
    for corner_z in [parent_z, parent_z + 1] {
        for corner_x in [parent_x, parent_x + 1] {
            let at = grid.volume.index_unchecked(
                cell(corner_x, 0, size.x),
                first,
                cell(corner_z, 0, size.z),
            );
            let mut previous = u32::MAX;
            for &id in &grid.ids[at..=at + (last - first) as usize] {
                let id = u32::from(id);
                if id != previous {
                    previous = id;
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
        }
    }
    true
}

fn grid_biome(grid: &BiomeGrid, quart: QuartPos) -> u32 {
    let min = grid.volume.min_block();
    let size = grid.volume.size();
    // A strip with no blocks at all starts its descent below the sections this
    // dispatch carries, which is the one lookup the grid does not span.
    let origin = QuartPos::of(min.into());
    let at = IVec3::new(quart.x - origin.x, quart.y - origin.y, quart.z - origin.z)
        .clamp(IVec3::ZERO, size - IVec3::ONE);
    u32::from(grid.get(at.x, at.y, at.z))
}

/// Writes through the strip's height, which the gradients of the strips after
/// it read. Carving the top block away lowers the height to the next block
/// below it, exactly as raising it does the other way.
pub(crate) fn set_block(
    column: &ColumnBlocks,
    tops: &mut [i32; 256],
    min_y: i32,
    x: i32,
    y: i32,
    z: i32,
    state: VoxelId,
) {
    if column.get(x, y, z).is_none() {
        return;
    }
    column.set(x, y, z, state);
    let air = VoxelId::default();
    let top = &mut tops[(z * 16 + x) as usize];
    if state != air {
        *top = (*top).max(y);
    } else if *top == y {
        *top = (min_y..y)
            .rev()
            .find(|&below| column.get(x, below, z).is_some_and(|old| old != air))
            .unwrap_or(NO_TOP);
    }
}

/// The pillars of eroded badlands, built before the rule pass so the rules
/// paint them.
#[allow(clippy::too_many_arguments)]
fn eroded_badlands(
    column: &ColumnBlocks,
    tops: &mut [i32; 256],
    program: &MaterialProgram,
    x: i32,
    z: i32,
    block_x: i32,
    block_z: i32,
    height: i32,
    min_y: i32,
    stone: VoxelId,
    fluid: VoxelId,
) {
    let air = VoxelId::default();
    let sample = |which, scale: f64| noise_2d(program, which, block_x, block_z, scale);
    // The reference mixes the widths deliberately: this product is a double and
    // the next is a float, and evaluating either in the other width moves the
    // pillar.
    let buffer = (f64::from(sample(SurfaceNoise::BadlandsSurface, 1.0)) * 8.25)
        .abs()
        .min(f64::from(sample(SurfaceNoise::BadlandsPillar, 0.2) * 15.0));
    if buffer <= 0.0 {
        return;
    }
    let roof = (f64::from(sample(SurfaceNoise::BadlandsPillarRoof, 0.75)) * 1.5).abs();
    let start_y = (64.0 + (buffer * buffer * 2.5).min((roof * 50.0).ceil() + 24.0)).floor() as i32;
    if height > start_y {
        return;
    }

    for y in (min_y..=start_y).rev() {
        match column.get(x, y, z) {
            Some(old) if old == stone => break,
            Some(old) if old == fluid => return,
            _ => {}
        }
    }
    for y in (min_y..=start_y).rev() {
        if column.get(x, y, z).unwrap_or(air) != air {
            break;
        }
        set_block(column, tops, min_y, x, y, z, stone);
    }
}

/// The icebergs of frozen ocean, built after the rule pass so the rules do not
/// touch them, and never where the carvers opened the column.
#[allow(clippy::too_many_arguments)]
fn frozen_ocean(
    column: &ColumnBlocks,
    tops: &mut [i32; 256],
    program: &MaterialProgram,
    min_surface_level: i32,
    min_y: i32,
    x: i32,
    z: i32,
    block_x: i32,
    block_z: i32,
    height: i32,
    sea_level: i32,
    fluid: VoxelId,
    ids: &SurfaceIds,
    carving: Option<&TerrainCarving<'_, '_>>,
) {
    let air = VoxelId::default();
    let sample = |which, scale: f64| noise_2d(program, which, block_x, block_z, scale);
    let set_block = |tops: &mut [i32; 256], y: i32, state: VoxelId| {
        if !carving.is_some_and(|carving| carving.is_carved(x, y, z)) {
            set_block(column, tops, min_y, x, y, z, state);
        }
    };
    let iceberg = (f64::from(sample(SurfaceNoise::IcebergSurface, 1.0)) * 8.25)
        .abs()
        .min(f64::from(sample(SurfaceNoise::IcebergPillar, 1.28) * 15.0));
    if iceberg <= 1.8 {
        return;
    }
    let roof = (f64::from(sample(SurfaceNoise::IcebergPillarRoof, 1.17)) * 1.5).abs();
    let mut top = (iceberg * iceberg * 1.2).min((roof * 40.0).ceil() + 14.0);
    // The reference lowers `top` by two where the surface biome is warm enough
    // to melt the iceberg slightly, which needs a per-biome temperature nothing
    // here can reach at generation time; until it can, every iceberg in deep
    // frozen ocean is two blocks taller than vanilla's.
    if top <= 2.0 {
        return;
    }

    let bottom = f64::from(sea_level) - top - 7.0;
    top += f64::from(sea_level);
    let mut random = program.noise_random_at(IVec3::new(block_x, 0, block_z));
    let max_snow_depth = 2 + random.next_i32_bound(4);
    let min_snow_height = sea_level + 18 + random.next_i32_bound(10);
    let mut snow_depth = 0;

    for y in (min_surface_level..=height.max(top as i32 + 1)).rev() {
        let old = column.get(x, y, z).unwrap_or(air);
        if old == air && y < top as i32 && random.next_f64() > 0.01
            || old == fluid && y > bottom as i32 && y < sea_level && random.next_f64() > 0.15
        {
            if snow_depth <= max_snow_depth && y > min_snow_height {
                set_block(tops, y, ids.snow_block);
                snow_depth += 1;
            } else {
                set_block(tops, y, ids.packed_ice);
            }
        }
    }
}

fn noise_2d(
    program: &MaterialProgram,
    which: SurfaceNoise,
    block_x: i32,
    block_z: i32,
    scale: f64,
) -> f32 {
    program.noise(program.surface_noise(which)).get(
        f64::from(block_x) * scale,
        0.0,
        f64::from(block_z) * scale,
    )
}
