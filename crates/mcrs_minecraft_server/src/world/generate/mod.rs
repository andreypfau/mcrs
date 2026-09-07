use crate::world::chunk::CancellationToken;
use crate::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use bevy_math::IVec3;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::beta_surface::beta_surface_blocks;
use mcrs_minecraft_world::biome::climate::TargetPoint;
use mcrs_minecraft_world::biome::source::{
    BetaLandBiome, BiomeSource, beta_biome_from_climate, beta_get_biome,
};
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_minecraft_worldgen::interval::Interval;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::NoiseRouter;
use mcrs_minecraft_worldgen::volume::Volume;
use mcrs_voxel_storage::VoxelId;

/// Margin the whole-cell fill keeps away from zero. `final_density_cell_bounds`
/// is f32 interval arithmetic without outward rounding, so a bound that lands
/// exactly on zero is not trustworthy; cells inside the margin go block by block.
const CELL_BOUNDS_SLACK: f32 = 1e-5;

/// The `interpolated` wrapper inputs at every cell corner of a whole chunk
/// column, laid out one `volume`-shaped row per wrapper.
struct CellLattice {
    volume: Volume,
    cell: IVec3,
    values: Vec<f32>,
    width: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CellFill {
    Solid,
    Fluid,
    Air,
    /// Empty, but crossing sea level: fluid below it, air above.
    Sea,
    Mixed,
}

impl CellLattice {
    /// `None` for a router with no single cell lattice, or one whose cells do
    /// not tile a section; such a chunk is filled block by block throughout.
    fn fill(
        noise_router: &NoiseRouter,
        block_x: i32,
        block_z: i32,
        ws: &mut Workspace,
    ) -> Option<Self> {
        let cell = noise_router.cell_size()?;
        // Sections must tile into whole cells, and the lattice must start on
        // one, or the eight values below would not be a cell's corners and the
        // interval bound over them would not hold.
        if 16 % cell.x != 0 || 16 % cell.y != 0 || 16 % cell.z != 0 {
            return None;
        }
        let height = noise_router.noise_height() as i32;
        if noise_router.noise_min_y() % 16 != 0 || height % 16 != 0 {
            return None;
        }
        let volume = Volume::new(
            IVec3::new(16 / cell.x + 1, height / cell.y + 1, 16 / cell.z + 1),
            IVec3::new(block_x, noise_router.noise_min_y(), block_z),
            cell,
        );
        let inputs = noise_router.cell_inputs();
        let mut values = vec![0.0f32; inputs.len() * volume.len()];
        noise_router.fill_nodes(ws, &volume, inputs, &mut values);
        noise_router.pin_cell_lattice(ws, &volume, &values);
        Some(Self {
            volume,
            cell,
            values,
            width: inputs.len(),
        })
    }

    /// The eight corner values of one cell, per wrapper.
    fn corner_bounds(&self, at: IVec3, out: &mut [Interval]) {
        let stride = self.volume.len();
        for (k, bound) in out.iter_mut().enumerate() {
            let row = &self.values[k * stride..(k + 1) * stride];
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for dz in 0..2 {
                for dx in 0..2 {
                    for dy in 0..2 {
                        let v = row[self.volume.index_unchecked(at.x + dx, at.y + dy, at.z + dz)];
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
            *bound = Interval::of(lo, hi);
        }
    }

    fn classify(
        &self,
        noise_router: &NoiseRouter,
        at: IVec3,
        sea_level: i32,
        fill: &mut FillBuffers,
    ) -> CellFill {
        self.corner_bounds(at, &mut fill.corners);
        let Some(bounds) = noise_router.final_density_cell_bounds(&fill.corners) else {
            return CellFill::Mixed;
        };
        if bounds.min() > CELL_BOUNDS_SLACK {
            return CellFill::Solid;
        }
        if bounds.max() < -CELL_BOUNDS_SLACK {
            let min_y = self.volume.block_y(at.y);
            if min_y + self.cell.y <= sea_level {
                return CellFill::Fluid;
            }
            if min_y >= sea_level {
                return CellFill::Air;
            }
            return CellFill::Sea;
        }
        CellFill::Mixed
    }
}

/// Buffers every fill in a chunk column reuses.
#[derive(Default)]
struct FillBuffers {
    ws: Workspace,
    density: Vec<f32>,
    corners: Vec<Interval>,
}

/// Place `final_density` over a whole chunk column.
///
/// The corner lattice is the pass-through case of the fill — every
/// `interpolated` wrapper hands its input straight back — so one fill over the
/// column settles most cells outright from interval arithmetic over their eight
/// corners, and only the rest are filled block by block.
///
/// Returns `false` if the column was cancelled part-way.
fn fill_column(
    column: &ColumnBlocks,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    cancel: &CancellationToken,
) -> bool {
    let sea_level = noise_router.sea_level();
    let default_block = noise_router.default_block_state();
    let default_fluid = noise_router.default_fluid_state();
    let mut fill = FillBuffers::default();

    let Some(lattice) = CellLattice::fill(noise_router, block_x, block_z, &mut fill.ws) else {
        return fill_column_dense(column, block_x, block_z, noise_router, &mut fill, cancel);
    };

    let cell = lattice.cell;
    fill.corners.resize(lattice.width, Interval::exact(0.0));

    for cell_z in 0..lattice.volume.size().z - 1 {
        if cancel.is_cancelled() {
            return false;
        }
        for cell_x in 0..lattice.volume.size().x - 1 {
            for cell_y in (0..lattice.volume.size().y - 1).rev() {
                let at = IVec3::new(cell_x, cell_y, cell_z);
                let world = IVec3::new(
                    lattice.volume.block_x(cell_x),
                    lattice.volume.block_y(cell_y),
                    lattice.volume.block_z(cell_z),
                );
                let Some(index) = column.section_index(world.y) else {
                    continue;
                };
                let base = IVec3::new(cell_x * cell.x, world.y.rem_euclid(16), cell_z * cell.z);
                match lattice.classify(noise_router, at, sea_level, &mut fill) {
                    CellFill::Solid => fill_cell_box(column, index, base, cell, default_block),
                    CellFill::Fluid => fill_cell_box(column, index, base, cell, default_fluid),
                    CellFill::Air => {}
                    CellFill::Sea => fill_cell_box(
                        column,
                        index,
                        base,
                        IVec3::new(cell.x, sea_level - world.y, cell.z),
                        default_fluid,
                    ),
                    CellFill::Mixed => fill_blocks(
                        column,
                        index,
                        &Volume::dense(cell, world),
                        base,
                        noise_router,
                        &mut fill,
                    ),
                }
            }
        }
    }
    true
}

/// The block-by-block fallback for a router whose cells do not tile a section.
fn fill_column_dense(
    column: &ColumnBlocks,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    fill: &mut FillBuffers,
    cancel: &CancellationToken,
) -> bool {
    let noise_min_y = noise_router.noise_min_y();
    let noise_max_y = noise_min_y + noise_router.noise_height() as i32;
    for (index, &section_y) in column.y_sections().iter().enumerate() {
        if cancel.is_cancelled() {
            return false;
        }
        let section_min_y = section_y * 16;
        if section_min_y >= noise_max_y || section_min_y + 16 <= noise_min_y {
            continue;
        }
        let volume = Volume::dense(
            IVec3::splat(16),
            IVec3::new(block_x, section_min_y, block_z),
        );
        fill_blocks(column, index, &volume, IVec3::ZERO, noise_router, fill);
    }
    true
}

fn fill_cell_box(column: &ColumnBlocks, index: usize, base: IVec3, cell: IVec3, state: VoxelId) {
    column.fill_box_in_section(
        index,
        base.x,
        base.x + cell.x,
        base.y,
        base.y + cell.y,
        base.z,
        base.z + cell.z,
        state,
    );
}

fn fill_blocks(
    column: &ColumnBlocks,
    index: usize,
    volume: &Volume,
    origin: IVec3,
    noise_router: &NoiseRouter,
    fill: &mut FillBuffers,
) {
    let sea_level = noise_router.sea_level();
    let default_block = noise_router.default_block_state();
    let default_fluid = noise_router.default_fluid_state();
    fill.density.clear();
    fill.density.resize(volume.len(), 0.0);
    noise_router.fill(
        &mut fill.ws,
        volume,
        noise_router.final_density(),
        &mut fill.density,
    );
    for z in 0..volume.size().z {
        for x in 0..volume.size().x {
            for y in (0..volume.size().y).rev() {
                let value = fill.density[volume.index_unchecked(x, y, z)];
                let (px, py, pz) = (origin.x + x, origin.y + y, origin.z + z);
                if value > 0.0 {
                    column.set_in_section(index, px, py, pz, default_block);
                } else if volume.block_y(y) < sea_level {
                    column.set_in_section(index, px, py, pz, default_fluid);
                }
            }
        }
    }
}

/// The (temperature, humidity) pair at each of the sixteen biome-cell columns
/// of a chunk.
fn beta_climate_cells(noise_router: &NoiseRouter, block_x: i32, block_z: i32) -> [(f32, f32); 16] {
    let volume = Volume::new(
        IVec3::new(4, 1, 4),
        IVec3::new(block_x, 0, block_z),
        IVec3::new(4, 1, 4),
    );
    let mut values = vec![0.0f32; 2 * volume.len()];
    noise_router.fill_roots(
        &mut Workspace::new(),
        &volume,
        &[noise_router.temperature(), noise_router.vegetation()],
        &mut values,
    );
    let mut cells = [(0.0f32, 0.0f32); 16];
    for cx in 0..4 {
        for cz in 0..4 {
            let source = volume.index_unchecked(cx, 0, cz);
            cells[(cx * 4 + cz) as usize] = (values[source], values[volume.len() + source]);
        }
    }
    cells
}

/// One `BiomePalette` per section of the column, empty when no source can
/// answer for the position.
///
/// The two sources answer at different resolutions: a Beta biome comes from
/// temperature and humidity at `(x, z)` alone, so one palette is cloned down
/// the column, while a multi-noise biome is sampled per 4x4x4 cell and every
/// section gets its own.
fn column_biome_palettes(
    noise_router: &NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    multi_noise: Option<&MultiNoiseBiomeTable>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> Vec<BiomePalette> {
    if let Some((BiomeSource::MultiNoise(_), _)) = biome_context {
        return match multi_noise {
            Some(table) => multi_noise_palettes(noise_router, table, block_x, block_z, y_sections),
            None => vec![BiomePalette::default(); y_sections.len()],
        };
    }
    vec![beta_biome_palette(noise_router, biome_context, block_x, block_z); y_sections.len()]
}

/// The climate at every 4x4x4 cell of the column, resolved to a biome.
///
/// The density program fills a whole strided volume in one pass, so asking for
/// the column's cells at once costs a fraction of evaluating them one by one.
pub(crate) fn multi_noise_palettes(
    noise_router: &NoiseRouter,
    table: &MultiNoiseBiomeTable,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> Vec<BiomePalette> {
    let (Some(&first), Some(&last)) = (y_sections.first(), y_sections.last()) else {
        return Vec::new();
    };
    let volume = Volume::new(
        IVec3::new(4, (last - first + 1) * 4, 4),
        IVec3::new(block_x, first * 16, block_z),
        IVec3::splat(4),
    );
    let roots = [
        noise_router.temperature(),
        noise_router.vegetation(),
        noise_router.continents(),
        noise_router.erosion(),
        noise_router.depth(),
        noise_router.ridges(),
    ];
    let points = volume.len();
    let mut values = vec![0.0f32; roots.len() * points];
    let mut ws = Workspace::new();
    noise_router.fill_roots(&mut ws, &volume, &roots, &mut values);

    y_sections
        .iter()
        .map(|&section_y| {
            let mut biomes = BiomePalette::default();
            let base_y = (section_y - first) * 4;
            for cx in 0..4 {
                for cy in 0..4 {
                    for cz in 0..4 {
                        let at = volume.index_unchecked(cx, base_y + cy, cz);
                        let target = TargetPoint::new(
                            values[at],
                            values[points + at],
                            values[2 * points + at],
                            values[3 * points + at],
                            values[4 * points + at],
                            values[5 * points + at],
                        );
                        biomes.set_cell(
                            cx as usize,
                            cy as usize,
                            cz as usize,
                            table.biome_at(target),
                        );
                    }
                }
            }
            biomes
        })
        .collect()
}

/// The `BiomePalette` every section of a Beta column shares.
fn beta_biome_palette(
    noise_router: &NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    block_x: i32,
    block_z: i32,
) -> BiomePalette {
    let mut biomes = BiomePalette::default();
    let Some((biome_source, biome_registry)) =
        biome_context.filter(|(src, _)| matches!(src, BiomeSource::Beta { .. }))
    else {
        return biomes;
    };
    let climate = beta_climate_cells(noise_router, block_x, block_z);
    for cx in 0..4usize {
        for cz in 0..4usize {
            let (temp, humidity) = climate[cx * 4 + cz];
            let location = biome_source.beta_biome_location(temp, humidity, false);
            let network_id = match biome_registry.by_location(location.as_str()) {
                Some(id) => id as u8,
                None => {
                    // Falling back to id 0 renders a plausible-but-wrong biome, so a
                    // registry that cannot resolve a preset's own biome is loud.
                    tracing::error!(biome = %location.as_str(), "beta biome not present in registry snapshot");
                    debug_assert!(false, "unresolved beta biome location");
                    0
                }
            };
            for cy in 0..4usize {
                biomes.set_cell(cx, cy, cz, network_id);
            }
        }
    }
    biomes
}

/// The per-chunk seed Beta derives for its population passes: two odd multipliers
/// drawn once from the world seed, dotted with the chunk coordinate.
///
/// Caves feed it the *neighbouring* chunk they are carving from, ores the chunk
/// itself, so the coordinate is a parameter rather than the chunk under work.
pub fn beta_chunk_seed(world_seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
    let mut rng = LegacyRandom::new(world_seed as u64);
    let x_multiplier = rng.next_java_long() / 2 * 2 + 1;
    let z_multiplier = rng.next_java_long() / 2 * 2 + 1;
    (chunk_x as i64)
        .wrapping_mul(x_multiplier)
        .wrapping_add((chunk_z as i64).wrapping_mul(z_multiplier))
        ^ world_seed
}

/// Generate all sections in a column.
///
/// `final_density` is filled once over the whole column, then walked as a single
/// z, x, descending-y sweep over its cells.
///
/// A column `cancel` stops part-way returns `None` for every section: one
/// column-wide fill leaves no section boundary at which a partial result is
/// meaningful.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "world::column_gen", skip_all)
)]
pub fn generate_column(
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    multi_noise: Option<&MultiNoiseBiomeTable>,
    cancel: &CancellationToken,
) -> Vec<Option<(BlockPalette, BiomePalette)>> {
    let mut column = ColumnBlocks::new(y_sections);
    let Some(biome_palettes) = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        y_sections,
        noise_router,
        biome_context,
        multi_noise,
        cancel,
    ) else {
        return vec![None; y_sections.len()];
    };

    column.into_sections(&biome_palettes)
}

/// Fill a column densely from the density graph, for every preset.
///
/// Beta is data here like any other preset: `beta.json` describes its terrain as
/// density functions, so it runs the same graph, the same cell fill and the same
/// packing as the overworld. `None` means the column was cancelled.
pub fn fill_column_dense_any(
    column: &mut ColumnBlocks,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    multi_noise: Option<&MultiNoiseBiomeTable>,
    cancel: &CancellationToken,
) -> Option<Vec<BiomePalette>> {
    let block_x = section_x * 16;
    let block_z = section_z * 16;
    let biome_palettes = column_biome_palettes(
        noise_router,
        biome_context,
        multi_noise,
        block_x,
        block_z,
        y_sections,
    );
    column.reset(y_sections);

    if !fill_column(column, block_x, block_z, noise_router, cancel) {
        return None;
    }
    Some(biome_palettes)
}

/// Apply the Beta surface pass to a generated chunk column.
///
/// Ports replaceBlocksForBiome from back2beta with a single per-chunk LegacyRandom
/// that drives both the surface depth, beach conditions, and the bedrock Y 0-4
/// probabilistic check — all interleaved in back2beta's exact column iteration order.
///
/// The caller seeds `rng` once per chunk with seed = chunkX*341873128712 + chunkZ*132897987541.
/// `rng` must be threaded across section calls so the stream is continuous.
pub fn apply_beta_surface(
    column: &ColumnBlocks,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    biome_source: &BiomeSource,
    blocks: &BlockDefinitions,
    rng: &mut LegacyRandom,
) {
    let Some(beta) = noise_router.beta_noises() else {
        return;
    };

    // Extract the quantized biome lookup from the biome source.
    // back2beta's replaceBlocksForBiome reads biomes via getBiomeFromLookup (quantized).
    let beta_lookup = match biome_source {
        BiomeSource::Beta { lookup, .. } => Some(lookup.as_ref()),
        _ => None,
    };

    let sea_level = noise_router.sea_level();
    let default_fluid = noise_router.default_fluid_state();
    let stone = noise_router.default_block_state();
    let bedrock = VoxelId::from(blocks.default_state("minecraft:bedrock"));
    let sandstone = VoxelId::from(blocks.default_state("minecraft:sandstone"));
    let gravel = VoxelId::from(blocks.default_state("minecraft:gravel"));
    let ice = VoxelId::from(blocks.default_state("minecraft:ice"));
    let sand = VoxelId::from(blocks.default_state("minecraft:sand"));

    const D0: f64 = 0.03125;

    // Three 16x16 grids, indexed geographically as x * 16 + z. Beta permutes the
    // axes: world Z goes to the noise's y argument and its own z is pinned to
    // zero, so the walk is x outer and world Z inner, which is the index order
    // the arrays want anyway.
    //
    //   r: `n.a(r, i*16, jj*16, 0.0, 16, 16, 1, d0, d0, 1.0)`       sand and gravel
    //   s: `n.a(s, i*16, 109.0134, jj*16, 16, 1, 16, d0, 1.0, d0)`  gravel override
    //   t: `o.a(t, i*16, jj*16, 0.0, 16, 16, 1, d0*2, d0*2, d0*2)`  surface depth
    //
    // r and t go through the grid fill because Beta's reuse of a lattice cell's
    // corner dots makes that fill depend on the grid and not only on the
    // position. `s` has ySize 1, so Beta takes the branch that drops the y
    // offset entirely — the 109.0134 never reaches the lattice — and that branch
    // is an ordinary function of x and z.
    let (mut r, mut t) = ([0.0f32; 256], [0.0f32; 256]);
    let offset = [block_x as f64, block_z as f64, 0.0];
    beta.beach
        .fill_legacy_grid(&mut r, offset, [16, 16, 1], [D0, D0, 1.0]);
    beta.surface
        .fill_legacy_grid(&mut t, offset, [16, 16, 1], [D0 * 2.0, D0 * 2.0, D0 * 2.0]);

    let mut s = [0.0f32; 256];
    for x in 0..16i32 {
        for z in 0..16i32 {
            let (wx, wz) = ((block_x + x) as f64, (block_z + z) as f64);
            s[(x * 16 + z) as usize] = beta.beach_flat.get(wx * D0, 0.0, wz * D0);
        }
    }

    let mut ws = Workspace::new();
    let (temperatures, humidities) =
        noise_router.sample_beta_climate_grids(&mut ws, block_x, block_z);

    // back2beta replaceBlocksForBiome: outer loop kk=0..16 is Z, inner ll=0..16 is X.
    // Noise arrays r/s/t are filled at index x*16+z (geographic) and read at ll*16+kk
    // = x*16+z — the same geographic index. Climate is sampled at geographic (wx, wz).
    for z_local in 0..16i32 {
        for x_local in 0..16i32 {
            let idx = (x_local * 16 + z_local) as usize;

            // Three RNG draws per column matching Java's Random.nextDouble() exactly.
            let flag = r[idx] as f64 + rng.next_java_double() * 0.2 > 0.0;
            let flag1 = s[idx] as f64 + rng.next_java_double() * 0.2 > 3.0;
            let i1 = (t[idx] as f64 / 3.0 + 3.0 + rng.next_java_double() * 0.25) as i32;

            let (temp, humidity) = (temperatures[idx], humidities[idx]);
            let biome_land: BetaLandBiome = if let Some(table) = beta_lookup {
                beta_biome_from_climate(table, temp, humidity)
            } else {
                beta_get_biome(temp, humidity)
            };
            let (top_block, filler_block) = beta_surface_blocks(biome_land, blocks);
            let (top_block, filler_block) = (VoxelId::from(top_block), VoxelId::from(filler_block));

            // j1 in back2beta: depth counter, -1 means "not yet in surface layer".
            let mut j1: i32 = -1;
            // Mutable top/filler for current Y zone (back2beta: b1, b2).
            let mut b1 = top_block;
            let mut b2 = filler_block;
            let air = VoxelId(0);

            // Sweep from world Y=127 down to 0 (back2beta: k1 = 127..=0).
            // Bedrock check is interleaved inside this loop.
            for k1 in (0i32..=127).rev() {
                // Bedrock check (back2beta: k1 <= 0 + this.j.nextInt(5)).
                if k1 <= rng.next_i32_bound(5) {
                    column.set(x_local, k1, z_local, bedrock);
                } else {
                    let current_id = column.get(x_local, k1, z_local);

                    let current_id = match current_id {
                        Some(id) => id,
                        None => continue, // section not present — skip
                    };

                    if current_id == air {
                        j1 = -1;
                    } else if current_id == stone {
                        if j1 == -1 {
                            if i1 <= 0 {
                                b1 = air;
                                b2 = stone;
                            } else if k1 >= sea_level - 4 && k1 <= sea_level + 1 {
                                b1 = top_block;
                                b2 = filler_block;
                                if flag1 {
                                    b1 = air;
                                }
                                if flag1 {
                                    b2 = gravel;
                                }
                                if flag {
                                    b1 = sand;
                                }
                                if flag {
                                    b2 = sand;
                                }
                            }

                            if k1 < sea_level && b1 == air {
                                b1 = default_fluid;
                            }

                            j1 = i1;
                            let place = if k1 >= sea_level - 1 { b1 } else { b2 };
                            column.set(x_local, k1, z_local, place);
                        } else if j1 > 0 {
                            j1 -= 1;
                            column.set(x_local, k1, z_local, b2);
                            if j1 == 0 && b2 == sand {
                                j1 = rng.next_i32_bound(4);
                                b2 = sandstone;
                            }
                        }
                    }
                }
            }

            // back2beta fillDensityTerrain: if d17 < 0.5 && Y == sea_level-1, replace
            // water with ice. No RNG consumed — must stay after all bedrock draws.
            if temp < 0.5 {
                let ice_y = sea_level - 1;
                if column.get(x_local, ice_y, z_local) == Some(default_fluid) {
                    column.set(x_local, ice_y, z_local, ice);
                }
            }
        }
    }
}

pub mod column_blocks;
pub use column_blocks::ColumnBlocks;
pub mod beta_caves;
pub use beta_caves::{BetaCaveBlockIds, apply_beta_caves};
pub mod beta_ores;
pub mod modern_carvers;
pub mod multi_noise_biomes;
pub use beta_ores::{BetaOreBlockIds, apply_beta_ores, place_all_ores};

#[cfg(test)]
mod tests;
