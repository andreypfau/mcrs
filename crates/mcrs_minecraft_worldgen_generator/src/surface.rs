use crate::modern_carvers::TerrainCarving;
use crate::stored_biomes::{present_biomes, stored_biome, stored_biomes_between};
use crate::{ColumnBlocks, NO_TOP};
use bevy_math::IVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_level::palette::BiomePalette;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_registry::shared::Resolved;
use mcrs_minecraft_registry::{Id, LoadReport, RegistrySet};
use mcrs_minecraft_worldgen_density::aquifer::WAY_BELOW_MIN_Y;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;
use mcrs_minecraft_worldgen_surface::{
    MaterialEval, MaterialScratch, NO_WATER, SettledState, SurfaceNoise,
};

/// The biomes the two hardcoded landforms name, which no rule does.
pub struct SurfaceIds {
    pub eroded_badlands: Id<keys::Biome>,
    pub frozen_ocean: Id<keys::Biome>,
    pub deep_frozen_ocean: Id<keys::Biome>,
}

impl SurfaceIds {
    pub fn resolve(set: &RegistrySet, report: &mut LoadReport) -> Option<Resolved<Self>> {
        let biomes = report.registry(set, keys::BIOME)?;
        let eroded_badlands = report.require(&biomes, &keys::biome::ERODED_BADLANDS);
        let frozen_ocean = report.require(&biomes, &keys::biome::FROZEN_OCEAN);
        let deep_frozen_ocean = report.require(&biomes, &keys::biome::DEEP_FROZEN_OCEAN);
        Some(Resolved::new(Self {
            eroded_badlands: eroded_badlands?,
            frozen_ocean: frozen_ocean?,
            deep_frozen_ocean: deep_frozen_ocean?,
        }))
    }
}

#[derive(Clone, Copy)]
pub struct SurfaceStates {
    pub snow_block: VoxelId,
    pub packed_ice: VoxelId,
    pub dirt: VoxelId,
}

impl SurfaceStates {
    pub fn new(blocks: &BlockDefinitions) -> Self {
        let state = |block: Id<keys::Block>| -> VoxelId { blocks.default_state_of(block).0.into() };
        Self {
            snow_block: state(keys::block::SNOW_BLOCK),
            packed_ice: state(keys::block::PACKED_ICE),
            dirt: state(keys::block::DIRT),
        }
    }
}

/// Whether a column buffer covers the dimension rather than a slice of it.
///
/// The descent starts one block above each strip's highest non-air block, and
/// that height is read off the sections the buffer holds. Given a slice, the cut
/// is what the descent enters at, with no depth above it — so the top of the
/// slice is surfaced as if it were the sky, and a sheet of grass over dirt lands
/// in the middle of the rock. The stored biomes and the ore-vein prefill region are
/// cut the same way, and the rules that lay the bedrock floor are never reached.
/// Sections past the noise range are the End's, which the dimension carries and
/// the noise does not fill.
pub fn spans_dimension(y_sections: &[i32], min_y: i32, height: i32) -> bool {
    let bottom = min_y >> 4;
    let top = bottom + (height >> 4);
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
    biomes: &[BiomePalette],
    first_section_y: i32,
    router: &NoiseRouter,
    program: &MaterialProgram,
    ids: &SurfaceIds,
    states: &SurfaceStates,
    scratch: &mut MaterialScratch,
    mut carving: Option<&mut TerrainCarving<'_, '_>>,
) {
    let block_x = section_x * 16;
    let block_z = section_z * 16;
    let min_y = router.noise.min_y;
    let sea_level = router.sea_level;
    let stone = router.default_block_state;
    let fluid = router.default_fluid_state;
    let lava = router.lava_state;

    let present = present_biomes(biomes);

    let top = tops.iter().copied().max().unwrap_or(NO_TOP).max(min_y);
    let mut eval = MaterialEval::new(
        router,
        program,
        scratch,
        |x, y, z| u16::from(stored_biome(biomes, first_section_y, x, y, z)),
        |_, _, lo, hi, out| {
            stored_biomes_between(biomes, first_section_y, lo, hi, out);
            true
        },
        block_x,
        block_z,
        top,
        &present,
    );

    let fill_tops = *tops;
    let mut settled = Vec::new();
    for x in 0..16 {
        for z in 0..16 {
            let (bx, bz) = (block_x + x, block_z + z);
            let starting_height = height_of(tops, x, z, min_y) + 1;
            let surface_biome = u16::from(stored_biome(
                biomes,
                first_section_y,
                bx,
                starting_height,
                bz,
            ));
            if surface_biome == ids.eroded_badlands.number() {
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
                            if state == Some(states.dirt) {
                                eval.update_y(1, depth_below, water_level, y);
                                state = eval.apply();
                            }
                            carved_top = false;
                        }
                        match state {
                            Some(state) if state == stone => {}
                            state => {
                                set_block(column, tops, min_y, x, y, z, state.unwrap_or_default())
                            }
                        }
                    }
                },
            );

            if surface_biome == ids.frozen_ocean.number()
                || surface_biome == ids.deep_frozen_ocean.number()
            {
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
                    states,
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
    states: &SurfaceStates,
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
                set_block(tops, y, states.snow_block);
                snow_depth += 1;
            } else {
                set_block(tops, y, states.packed_ice);
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
