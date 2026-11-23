use crate::ids::require_id;
use crate::modern_carvers::TerrainCarving;
use crate::stored_biomes::{present_biomes, stored_biome, stored_biomes_between};
use crate::{ColumnBlocks, NO_TOP};
use glam::IVec3;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_chunk::palette::BiomePalette;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_registries::{Diagnostics, Registries};
use mcrs_minecraft_resource::{Id, ResourceKey};
use mcrs_minecraft_worldgen_density::aquifer::WAY_BELOW_MIN_Y;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_surface::compile::MaterialProgram;
use mcrs_minecraft_worldgen_surface::{
    MaterialEval, MaterialScratch, NO_WATER, SettledState, SurfaceNoise,
};

pub struct SurfaceIds {
    pub eroded_badlands: Id<Biome>,
    pub frozen_ocean: Id<Biome>,
    pub deep_frozen_ocean: Id<Biome>,
}

impl SurfaceIds {
    pub fn resolve(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Self> {
        Self::from_lookup(|key| require_id(registries, key, diagnostics))
    }

    pub fn from_lookup(
        mut biome: impl FnMut(ResourceKey<'static, Biome>) -> Option<Id<Biome>>,
    ) -> Option<Self> {
        let eroded_badlands = biome(mcrs_minecraft_biome::biome_keys::ERODED_BADLANDS);
        let frozen_ocean = biome(mcrs_minecraft_biome::biome_keys::FROZEN_OCEAN);
        let deep_frozen_ocean = biome(mcrs_minecraft_biome::biome_keys::DEEP_FROZEN_OCEAN);
        Some(Self {
            eroded_badlands: eroded_badlands?,
            frozen_ocean: frozen_ocean?,
            deep_frozen_ocean: deep_frozen_ocean?,
        })
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
        let state = |block: Id<mcrs_minecraft_block::block_keys::Block>| -> VoxelId {
            blocks.default_state_of(block).number().into()
        };
        Self {
            snow_block: state(mcrs_minecraft_block::block_keys::Block::SnowBlock.id()),
            packed_ice: state(mcrs_minecraft_block::block_keys::Block::PackedIce.id()),
            dirt: state(mcrs_minecraft_block::block_keys::Block::Dirt.id()),
        }
    }
}

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

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one surface pass"
)]
#[expect(
    clippy::too_many_lines,
    reason = "rewrites every strip of the column from the top down in one pass"
)]
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
    let lava = router.lava_state();

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
            let mut run_bottom = min_y;
            let mut carved_top = false;
            let mut next_carved = carving
                .as_deref()
                .and_then(|carving| carving.highest_carved(x, z, min_y, height));
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
                        run_bottom = bottom;
                        Descent::Step
                    }
                    Visit::OpenRun { top, bottom } => {
                        if carved_top {
                            carved_top = carving.as_deref().is_some_and(|carving| {
                                (bottom..=top).all(|y| carving.is_carved(x, y, z))
                            });
                        }
                        Descent::Step
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
                        let mut span = None;
                        let mut state = if run < settled.len() && y <= settled[run].1 {
                            match settled[run].2 {
                                SettledState::Block(state) => {
                                    span = Some(
                                        settled[run]
                                            .0
                                            .max(run_bottom)
                                            .max(column.present_bottom(y)),
                                    );
                                    state
                                }
                                SettledState::Bandlands => eval.bandlands_at(y),
                                SettledState::Resume(guard) => {
                                    eval.update_y(depth_above, depth_below, water_level, y);
                                    eval.apply_from(guard)
                                }
                            }
                        } else {
                            eval.update_y(depth_above, depth_below, water_level, y);
                            eval.apply()
                        };
                        if next_carved.is_some_and(|carved| y < carved) {
                            next_carved = carving
                                .as_deref()
                                .and_then(|carving| carving.highest_carved(x, z, min_y, y));
                        }
                        if next_carved == Some(y)
                            && let Some(substance) = carving
                                .as_deref_mut()
                                .and_then(|carving| carving.substance(x, y, z, state))
                        {
                            carved_top |= depth_above == 1;
                            set_block(column, tops, min_y, x, y, z, substance);
                            return Descent::Step;
                        }
                        if carved_top {
                            if state == Some(states.dirt) {
                                eval.update_y(1, depth_below, water_level, y);
                                state = eval.apply();
                                span = None;
                            }
                            carved_top = false;
                        }
                        match state {
                            Some(state) if state == stone => {}
                            state => {
                                set_block(column, tops, min_y, x, y, z, state.unwrap_or_default());
                            }
                        }
                        match (span, state) {
                            (Some(bottom), Some(state))
                                if bottom < y && state != VoxelId::default() =>
                            {
                                let span = Span {
                                    x,
                                    z,
                                    bottom,
                                    top: y - 1,
                                };
                                write_span(
                                    column,
                                    tops,
                                    min_y,
                                    carving.as_deref_mut(),
                                    span,
                                    state,
                                    stone,
                                );
                                Descent::Through(bottom)
                            }
                            _ => Descent::Step,
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
    OpenRun {
        top: i32,
        bottom: i32,
    },
}

pub(crate) enum Descent {
    Step,
    Through(i32),
}

pub(crate) fn descend_strip(
    column: &ColumnBlocks,
    x: i32,
    z: i32,
    height: i32,
    min_y: i32,
    fluids: [VoxelId; 2],
    mut visit: impl FnMut(Visit) -> Descent,
) {
    let air = VoxelId::default();
    let is_fluid = |state: VoxelId| fluids.contains(&state);
    let mut depth_above = 0;
    let mut water_level = NO_WATER;
    let mut next_ceiling = i32::MAX;

    let mut y = height;
    while y >= min_y {
        let Some(old) = column.get(x, y, z) else {
            let Some(below) = y.checked_sub(1) else {
                return;
            };
            y = below;
            continue;
        };
        if old == air {
            depth_above = 0;
            water_level = NO_WATER;
            let bottom = column.run_bottom(x, z, y, min_y, old);
            visit(Visit::OpenRun { top: y, bottom });
            y = bottom;
        } else if is_fluid(old) {
            if water_level == NO_WATER {
                water_level = y + 1;
            }
            let bottom = column.run_bottom(x, z, y, min_y, old);
            visit(Visit::OpenRun { top: y, bottom });
            y = bottom;
        } else {
            if next_ceiling >= y {
                next_ceiling = column
                    .highest_below(x, z, y, min_y, |state| state == air || is_fluid(state))
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
            let descent = visit(Visit::Block {
                y,
                depth_above,
                depth_below: y - next_ceiling + 1,
                water_level,
            });
            if let Descent::Through(bottom) = descent {
                let bottom = bottom
                    .max(next_ceiling.max(min_y).max(column.present_bottom(y)))
                    .min(y);
                depth_above += y - bottom;
                y = bottom;
            }
        }
        let Some(below) = y.checked_sub(1) else {
            return;
        };
        y = below;
    }
}

struct Span {
    x: i32,
    z: i32,
    bottom: i32,
    top: i32,
}

fn write_span(
    column: &ColumnBlocks,
    tops: &mut [i32; 256],
    min_y: i32,
    mut carving: Option<&mut TerrainCarving<'_, '_>>,
    span: Span,
    state: VoxelId,
    stone: VoxelId,
) {
    let Span { x, z, bottom, top } = span;
    let mut upper = top;
    while upper >= bottom {
        let carved = carving
            .as_deref()
            .and_then(|carving| carving.highest_carved(x, z, bottom, upper));
        let lower = carved.map_or(bottom, |carved| carved + 1);
        if state != stone && lower <= upper {
            column.fill_strip(x, z, lower, upper, state);
            if let Some(height) = tops.get_mut((z * 16 + x) as usize) {
                *height = (*height).max(upper);
            }
        }
        let Some(carved) = carved else {
            return;
        };
        match carving
            .as_deref_mut()
            .and_then(|carving| carving.substance(x, carved, z, Some(state)))
        {
            Some(substance) => set_block(column, tops, min_y, x, carved, z, substance),
            None if state != stone => set_block(column, tops, min_y, x, carved, z, state),
            None => {}
        }
        upper = carved - 1;
    }
}

fn height_of(tops: &[i32; 256], x: i32, z: i32, min_y: i32) -> i32 {
    match tops[(z * 16 + x) as usize] {
        NO_TOP => min_y - 1,
        top => top,
    }
}

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

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one eroded-badlands pillar"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one frozen-ocean iceberg"
)]
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
    program.surface_noise_sampler(which).get(
        f64::from(block_x) * scale,
        0.0,
        f64::from(block_z) * scale,
    )
}
