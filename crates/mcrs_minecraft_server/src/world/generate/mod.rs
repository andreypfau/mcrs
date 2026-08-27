use crate::world::chunk::CancellationToken;
use bevy_math::IVec3;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_protocol::BlockStateId;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::beta_surface::beta_surface_blocks;
use mcrs_minecraft_world::biome::source::{
    BetaLandBiome, BiomeSource, beta_biome_from_climate, beta_get_biome,
};
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_minecraft_worldgen::density_function::{
    ColumnCache, FillScratch, NoiseRouter, Volume, beta_terrain_f64::BetaTerrainF64,
};
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;

/// Margin the whole-cell fill keeps away from zero. `final_density_cell_bounds`
/// is f32 interval arithmetic without outward rounding, so a bound that lands
/// exactly on zero is not trustworthy; cells inside the margin go block by block.
const CELL_BOUNDS_SLACK: f32 = 1e-5;

/// The `interpolated` wrapper inputs at every cell corner of a whole chunk
/// column, laid out one `volume`-shaped row per wrapper.
struct ColumnCorners {
    volume: Volume,
    cell: IVec3,
    values: Vec<f32>,
    width: usize,
}

impl ColumnCorners {
    /// `None` for a router with no single cell lattice, or one whose cells do
    /// not tile a section; such a chunk is filled block by block throughout.
    fn fill(
        noise_router: &NoiseRouter,
        block_x: i32,
        block_z: i32,
        scratch: &mut FillScratch,
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
        let rows = height / cell.y + 1;
        let volume = Volume::new(
            IVec3::new(16 / cell.x + 1, rows, 16 / cell.z + 1),
            IVec3::new(block_x, noise_router.noise_min_y(), block_z),
            cell,
        );
        let roots = noise_router.cell_value_roots();
        let mut values = vec![0.0f32; roots.len() * volume.len()];
        noise_router.fill_roots(roots, &volume, &mut values, scratch);
        Some(Self {
            volume,
            cell,
            values,
            width: roots.len(),
        })
    }

    /// The eight corner values of one cell, per wrapper.
    fn cell_bounds(&self, cell: IVec3, out: &mut [(f32, f32)]) {
        let stride = self.volume.len();
        for (k, bound) in out.iter_mut().enumerate() {
            let row = &self.values[k * stride..(k + 1) * stride];
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for dz in 0..2 {
                for dx in 0..2 {
                    for dy in 0..2 {
                        let v =
                            row[self
                                .volume
                                .index_unchecked(cell.x + dx, cell.y + dy, cell.z + dz)];
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
            *bound = (lo, hi);
        }
    }
}

/// Buffers every section fill in a chunk column reuses.
#[derive(Default)]
struct SectionFill {
    scratch: FillScratch,
    density: Vec<f32>,
    bounds: Vec<(f32, f32)>,
    wrapper_bounds: Vec<(f32, f32)>,
}

/// Generate a single section by filling `final_density` over it.
///
/// The corner lattice is the pass-through case of the fill — every
/// `interpolated` wrapper hands its input straight back — so interval
/// arithmetic over a cell's eight corners settles most cells outright, and only
/// the rest are filled block by block.
fn generate_section(
    section_min: IVec3,
    block_states: &mut BlockPalette,
    noise_router: &NoiseRouter,
    corners: Option<&ColumnCorners>,
    fill: &mut SectionFill,
) {
    let block_y = section_min.y;
    let sea_level = noise_router.sea_level();
    let default_block = noise_router.default_block_state();
    let default_fluid = noise_router.default_fluid_state();

    let Some(corners) = corners else {
        let volume = Volume::dense(IVec3::splat(16), section_min);
        fill_and_set(
            block_states,
            &volume,
            IVec3::ZERO,
            noise_router,
            fill,
            sea_level,
            default_block,
            default_fluid,
        );
        return;
    };

    let cell = corners.cell;
    let row0 = (block_y - corners.volume.min_block_y()) / cell.y;
    fill.bounds.clear();
    fill.bounds
        .resize(noise_router.final_density_index() + 1, (0.0, 0.0));
    fill.wrapper_bounds.clear();
    fill.wrapper_bounds.resize(corners.width, (0.0, 0.0));

    for cell_z in 0..16 / cell.z {
        for cell_x in 0..16 / cell.x {
            for cell_y in 0..16 / cell.y {
                let base = IVec3::new(cell_x * cell.x, cell_y * cell.y, cell_z * cell.z);
                debug_assert_eq!(
                    corners.volume.index_of_block(
                        section_min.x + base.x,
                        section_min.y + base.y,
                        section_min.z + base.z
                    ),
                    Some(
                        corners
                            .volume
                            .index_unchecked(cell_x, row0 + cell_y, cell_z)
                    ),
                    "cell corner is not the lattice position it indexes"
                );
                corners.cell_bounds(
                    IVec3::new(cell_x, row0 + cell_y, cell_z),
                    &mut fill.wrapper_bounds,
                );

                let cell_min_world_y = block_y + base.y;
                let cell_max_world_y = cell_min_world_y + cell.y;
                match noise_router.final_density_cell_bounds(&fill.wrapper_bounds, &mut fill.bounds)
                {
                    Some((lo, _)) if lo > CELL_BOUNDS_SLACK => {
                        fill_cell_box(block_states, base, cell, default_block);
                        continue;
                    }
                    Some((_, hi)) if hi < -CELL_BOUNDS_SLACK => {
                        if cell_max_world_y <= sea_level {
                            fill_cell_box(block_states, base, cell, default_fluid);
                            continue;
                        } else if cell_min_world_y >= sea_level {
                            continue;
                        }
                    }
                    _ => {}
                }

                let volume = Volume::dense(cell, section_min + base);
                fill_and_set(
                    block_states,
                    &volume,
                    base,
                    noise_router,
                    fill,
                    sea_level,
                    default_block,
                    default_fluid,
                );
            }
        }
    }
}

fn fill_cell_box(block_states: &mut BlockPalette, base: IVec3, cell: IVec3, state: VoxelId) {
    block_states.fill_box(
        base.x as usize,
        (base.x + cell.x) as usize,
        base.y as usize,
        (base.y + cell.y) as usize,
        base.z as usize,
        (base.z + cell.z) as usize,
        state,
    );
}

#[allow(clippy::too_many_arguments)]
fn fill_and_set(
    block_states: &mut BlockPalette,
    volume: &Volume,
    origin: IVec3,
    noise_router: &NoiseRouter,
    fill: &mut SectionFill,
    sea_level: i32,
    default_block: VoxelId,
    default_fluid: VoxelId,
) {
    fill.density.clear();
    fill.density.resize(volume.len(), 0.0);
    noise_router.fill(
        noise_router.final_density_index(),
        volume,
        &mut fill.density,
        &mut fill.scratch,
    );
    for z in 0..volume.size_z() {
        for x in 0..volume.size_x() {
            for y in 0..volume.size_y() {
                let value = fill.density[volume.index_unchecked(x, y, z)];
                let pos = BlockPos::new(origin.x + x, origin.y + y, origin.z + z);
                if value > 0.0 {
                    block_states.set(pos, default_block);
                } else if volume.block_y(y) < sea_level {
                    block_states.set(pos, default_fluid);
                }
            }
        }
    }
}

/// Fill a `BiomePalette` for a single 16x16x16 section from Beta climate data.
///
/// Each of the 4x4x4 biome cells is sampled once from the pre-populated
/// `column_cache`. The ocean/land split is per cell-row Y: a cell whose
/// center world Y falls below `sea_level` receives the ocean biome for
/// the land bucket at that XZ position; cells at or above sea level receive
/// the land biome directly.
///
/// A biome handle that is absent from the frozen registry snapshot signals a
/// misconfiguration (unregistered biome, asset load failure, or registry/preset
/// ordering bug). Such a miss is logged and asserted in debug builds rather than
/// silently substituting id 0, which would render a plausible-but-wrong biome.
fn fill_biome_palette_beta(
    biomes: &mut BiomePalette,
    _section_y: i32,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    column_cache: &ColumnCache,
    biome_source: &BiomeSource,
    biome_registry: &RegistrySnapshot<Biome>,
) {
    // Beta biomes are 2D: WorldChunkManager derives the biome purely from
    // temperature/humidity at (x,z) via getBiomeFromLookup, with no Y or
    // sea-level dependence, so every cell in a column shares one biome.
    for cx in 0..4usize {
        let sample_x = block_x + cx as i32 * 4;
        for cz in 0..4usize {
            let sample_z = block_z + cz as i32 * 4;
            let (temp, humidity) = noise_router.sample_climate_at(column_cache, sample_x, sample_z);
            let location = biome_source.beta_biome_location(temp, humidity, false);
            let network_id = match biome_registry.by_location(location.as_str()) {
                Some(id) => id as u8,
                None => {
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
}

/// Fill section block palettes for the Beta terrain using the exact-precision f64 path.
///
/// Runs `BetaTerrainF64::compute_density` + `fill_terrain` once for the whole 16×128×16
/// column, then distributes the flat block array into the requested Y sections.
/// Ice at sea_level-1 is placed here (matching Java's fillDensityTerrain), so the later
/// apply_beta_surface ice-placement is still correct (it only replaces water→ice).
fn fill_sections_beta_f64(
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &NoiseRouter,
    terrain: &BetaTerrainF64,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    blocks: &BlockDefinitions,
    cancel: &CancellationToken,
) -> Vec<Option<(BlockPalette, BiomePalette)>> {
    let block_x = section_x * 16;
    let block_z = section_z * 16;

    let sea_level = noise_router.sea_level();
    let stone_id = noise_router.default_block_state().0 as u32;
    let water_id = noise_router.default_fluid_state().0 as u32;
    let ice_id = blocks.default_state("minecraft:ice").0 as u32;
    let _air_id = 0u32;

    // Sample the 16×16 climate grids needed by computeDensity.
    let (temp_grid, rain_grid) = noise_router.sample_beta_climate_grids(block_x, block_z);

    // Run the f64 density computation and block fill.
    let density = terrain.compute_density(section_x, section_z, &temp_grid, &rain_grid);
    let flat =
        BetaTerrainF64::fill_terrain(&density, &temp_grid, sea_level, stone_id, water_id, ice_id);

    // Build a column cache for biome sampling (used by fill_biome_palette_beta).
    let mut column_cache = noise_router.new_column_cache(block_x, block_z);
    noise_router.populate_columns(&mut column_cache);

    let beta_biome = biome_context.and_then(|(src, reg)| {
        if matches!(src, BiomeSource::Beta { .. }) {
            Some((src, reg))
        } else {
            None
        }
    });

    y_sections
        .iter()
        .map(|&sy| {
            if cancel.is_cancelled() {
                return None;
            }

            let section_min_y = sy * 16;

            let mut blocks = BlockPalette::default();
            let mut biomes = BiomePalette::default();

            // Only sections in [0, 128) contain Beta terrain blocks.
            if (0..128).contains(&section_min_y) {
                for local_y in 0..16i32 {
                    let world_y = section_min_y + local_y;
                    if world_y >= 128 {
                        break;
                    }
                    for local_x in 0..16i32 {
                        for local_z in 0..16i32 {
                            let flat_idx = (local_x as usize) * 16 * 128
                                + (local_z as usize) * 128
                                + world_y as usize;
                            let block_u32 = flat[flat_idx];
                            if block_u32 != 0 {
                                blocks.set(
                                    BlockPos::new(local_x, local_y, local_z),
                                    BlockStateId(block_u32 as u16).into(),
                                );
                            }
                        }
                    }
                }
            }

            if let Some((src, reg)) = beta_biome {
                fill_biome_palette_beta(
                    &mut biomes,
                    sy,
                    block_x,
                    block_z,
                    noise_router,
                    &column_cache,
                    src,
                    reg,
                );
            }

            Some((blocks, biomes))
        })
        .collect()
}

/// Generate all sections in a column using a pre-populated ColumnCache.
/// Zone A (column-only density functions) is computed once for all 17x17 XZ positions
/// and reused across all Y sections, eliminating per-block column-change branches.
///
/// Adjacent Y sections share cell corners at their boundary via Y-boundary reuse,
/// eliminating ~33% of density evaluations for all sections after the first.
///
/// Accepts a `CancellationToken` for cooperative cancellation. The token is checked
/// between section generations; if cancelled, remaining sections return `None` while
/// already-completed sections return `Some((blocks, biomes))`.
///
/// When `biome_context` is `Some((source, registry))` and `source` is a Beta biome
/// source, every section's `BiomePalette` is filled from climate data.  Non-Beta
/// sources leave the palette as the default (id 0) — modern biome assignment is
/// unchanged.
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
    blocks: &BlockDefinitions,
    cancel: &CancellationToken,
) -> Vec<Option<(BlockPalette, BiomePalette)>> {
    // Beta path: use exact f64 density computation instead of the f32 density tree.
    if let Some(terrain) = noise_router.beta_terrain_f64() {
        return fill_sections_beta_f64(
            section_x,
            section_z,
            y_sections,
            noise_router,
            terrain,
            biome_context,
            blocks,
            cancel,
        );
    }

    let block_x = section_x * 16;
    let block_z = section_z * 16;

    let noise_min_y = noise_router.noise_min_y();
    let noise_max_y = noise_min_y + noise_router.noise_height() as i32;

    // Only fill biome palettes when the source is Beta; modern paths keep default().
    let beta_biome = biome_context.and_then(|(src, reg)| {
        if matches!(src, BiomeSource::Beta { .. }) {
            Some((src, reg))
        } else {
            None
        }
    });

    // The column grid now serves the Beta climate lookup alone; the density fill
    // walks its own columns.
    let column_cache = beta_biome.map(|_| {
        let mut cache = noise_router.new_column_cache(block_x, block_z);
        noise_router.populate_columns(&mut cache);
        cache
    });

    let mut fill = SectionFill::default();
    let corners = ColumnCorners::fill(noise_router, block_x, block_z, &mut fill.scratch);
    y_sections
        .iter()
        .map(|&sy| {
            // Check cancellation between sections (cooperative cancellation)
            if cancel.is_cancelled() {
                return None;
            }

            // Sections outside [noise_min_y, noise_min_y + noise_height) are always air.
            // This matches vanilla: only cells within the noise settings vertical range are
            // filled by the density function; everything else is the default block (air).
            // Clients still need biome data for these sections, so the palette is always filled
            // when a Beta biome source is active.
            let section_min_y = sy * 16;
            let section_max_y = section_min_y + 16;
            let mut blocks = BlockPalette::default();
            let mut biomes = BiomePalette::default();
            if section_min_y < noise_max_y && section_max_y > noise_min_y {
                generate_section(
                    IVec3::new(block_x, section_min_y, block_z),
                    &mut blocks,
                    noise_router,
                    corners.as_ref(),
                    &mut fill,
                );
            }
            if let Some(((src, reg), cache)) = beta_biome.zip(column_cache.as_ref()) {
                fill_biome_palette_beta(
                    &mut biomes,
                    sy,
                    block_x,
                    block_z,
                    noise_router,
                    cache,
                    src,
                    reg,
                );
            }
            Some((blocks, biomes))
        })
        .collect()
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
    sections: &mut Vec<Option<(BlockPalette, BiomePalette)>>,
    y_sections: &[i32],
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    biome_source: &BiomeSource,
    blocks: &BlockDefinitions,
    rng: &mut LegacyRandom,
) {
    let Some(beach_noise) = noise_router.beta_beach_noise() else {
        return;
    };
    let Some(surf_noise) = noise_router.beta_surface_noise() else {
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

    // Pre-sample noise arrays for the 16x16 chunk footprint using Java-exact bulk fill.
    // r[x*16+z]: beach XZ noise (gravel/sand condition).
    // s[x*16+z]: beach noise at Y=109 (gravel override condition).
    // t[x*16+z]: surface depth noise.
    //
    // Java call: n.a(r, i*16, jj*16, 0.0, 16, 16, 1, d0, d0, 1.0)
    // → fill_3d_bulk(x_start=block_x, y_start=block_z, z_start=0, x=16, y=16, z=1, sx=D0, sy=D0, sz=1.0)
    // Output index j1*16+k4 = x_local*16+z_local = x*16+z. ✓
    let mut r = [0.0f64; 256];
    beach_noise.fill_3d_bulk(
        &mut r,
        block_x as f64,
        block_z as f64,
        0.0,
        16,
        16,
        1,
        D0,
        D0,
        1.0,
    );

    // Java call: n.a(s, i*16, 109.0134, jj*16, 16, 1, 16, d0, 1.0, d0)
    // j=ySize=1: uses ySize==1 branch with y-lattice pinned to floor(109.0134*freq+oy).
    // Per-point via sample_xyz_beta is sufficient for the gravel-only flag1 condition.
    let mut s = [0.0f64; 256];
    for x in 0..16usize {
        for z in 0..16usize {
            s[x * 16 + z] = beach_noise.sample_xyz_beta(
                (block_x + x as i32) as f64,
                109.0134,
                (block_z + z as i32) as f64,
                D0,
                1.0,
                D0,
            );
        }
    }

    // Java call: o.a(t, i*16, jj*16, 0.0, 16, 16, 1, d0*2, d0*2, d0*2)
    // → fill_3d_bulk(x_start=block_x, y_start=block_z, z_start=0, x=16, y=16, z=1,
    //                sx=D0*2, sy=D0*2, sz=D0*2)
    let mut t = [0.0f64; 256];
    surf_noise.fill_3d_bulk(
        &mut t,
        block_x as f64,
        block_z as f64,
        0.0,
        16,
        16,
        1,
        D0 * 2.0,
        D0 * 2.0,
        D0 * 2.0,
    );

    // back2beta replaceBlocksForBiome: outer loop kk=0..16 is Z, inner ll=0..16 is X.
    // Noise arrays r/s/t are filled at index x*16+z (geographic) and read at ll*16+kk
    // = x*16+z — the same geographic index. Climate is sampled at geographic (wx, wz).
    for z_local in 0..16i32 {
        for x_local in 0..16i32 {
            let idx = (x_local * 16 + z_local) as usize;

            // Three RNG draws per column matching Java's Random.nextDouble() exactly.
            let flag = r[idx] + rng.next_java_double() * 0.2 > 0.0;
            let flag1 = s[idx] + rng.next_java_double() * 0.2 > 3.0;
            let i1 = (t[idx] / 3.0 + 3.0 + rng.next_java_double() * 0.25) as i32;

            let climate_x = block_x + x_local;
            let climate_z = block_z + z_local;
            let (temp, humidity) = noise_router.sample_beta_climate(climate_x, climate_z);
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
                let section_y = k1 >> 4;
                let local_y = k1 & 0xF;
                let si = y_sections.iter().position(|&sy| sy == section_y);

                // Bedrock check (back2beta: k1 <= 0 + this.j.nextInt(5)).
                if k1 <= rng.next_i32_bound(5) {
                    if let Some(si) = si
                        && let Some((blocks, _)) = sections[si].as_mut()
                    {
                        blocks.set(BlockPos::new(x_local, local_y, z_local), bedrock);
                    }
                } else {
                    let current_id = si
                        .and_then(|si| sections[si].as_ref())
                        .map(|(blocks, _)| blocks.get(BlockPos::new(x_local, local_y, z_local)));

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
                            if let Some(si) = si
                                && let Some((blocks, _)) = sections[si].as_mut()
                            {
                                let place = if k1 >= sea_level - 1 { b1 } else { b2 };
                                blocks.set(BlockPos::new(x_local, local_y, z_local), place);
                            }
                        } else if j1 > 0 {
                            j1 -= 1;
                            if let Some(si) = si
                                && let Some((blocks, _)) = sections[si].as_mut()
                            {
                                blocks.set(BlockPos::new(x_local, local_y, z_local), b2);
                            }
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
                let ice_section_y = ice_y >> 4;
                let ice_local_y = ice_y & 0xF;
                if let Some(si) = y_sections.iter().position(|&sy| sy == ice_section_y)
                    && let Some((blocks, _)) = sections[si].as_mut()
                {
                    let current = blocks.get(BlockPos::new(x_local, ice_local_y, z_local));
                    if current == default_fluid {
                        blocks.set(BlockPos::new(x_local, ice_local_y, z_local), ice);
                    }
                }
            }
        }
    }
}

pub mod beta_caves;
pub use beta_caves::{BetaCaveBlockIds, apply_beta_caves};
pub mod beta_ores;
pub use beta_ores::{BetaOreBlockIds, apply_beta_ores, place_all_ores};

#[cfg(test)]
mod tests;
