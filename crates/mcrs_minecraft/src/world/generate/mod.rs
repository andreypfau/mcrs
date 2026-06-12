use crate::world::chunk::CancellationToken;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_core::RegistrySnapshot;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_worldgen::density_function::{
    ColumnCache, NoiseRouter, NoiseCellInterpolator,
};
use mcrs_random::legacy::LegacyRandom;
use mcrs_random::Random;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::biome::beta_surface::beta_surface_blocks;
use mcrs_vanilla::biome::source::{BetaLandBiome, BiomeSource, beta_biome_from_climate, beta_get_biome};
use mcrs_vanilla::block::minecraft;

/// Generate a single section using a pre-populated column cache and interpolator.
/// The column cache and interpolator are passed in so they can be reused across
/// multiple Y sections in the same column.
///
/// Uses `fill_plane_cached_reuse` for Y-boundary sharing: the top-Y row of the
/// previous section is reused as the bottom-Y row of this section, eliminating
/// ~33% of density evaluations for all sections after the first.
fn generate_section(
    block_x: i32,
    block_y: i32,
    block_z: i32,
    block_states: &mut BlockPalette,
    noise_router: &NoiseRouter,
    column_cache: &mut ColumnCache,
    interp: &mut NoiseCellInterpolator,
) {
    let h_cell_blocks = interp.h_cell_blocks();
    let v_cell_blocks = interp.v_cell_blocks();
    let h_cells = interp.h_cells();
    let v_cells = interp.v_cells();

    let sea_level = noise_router.sea_level();
    let default_block = noise_router.default_block_state();
    let default_fluid = noise_router.default_fluid_state();

    // Fill the initial X start plane using column cache (with Y-boundary reuse)
    interp.fill_plane_cached_reuse(
        0,
        true,
        block_x,
        block_y,
        block_z,
        noise_router,
        column_cache,
    );

    for cell_x in 0..h_cells {
        // Fill end plane at x = block_x + (cell_x + 1) * h_cell_blocks
        let next_x = block_x + ((cell_x + 1) * h_cell_blocks) as i32;
        interp.fill_plane_cached_reuse(
            cell_x + 1,
            false,
            next_x,
            block_y,
            block_z,
            noise_router,
            column_cache,
        );

        for cell_z in 0..h_cells {
            for cell_y in (0..v_cells).rev() {
                interp.on_sampled_cell_corners(cell_y, cell_z);

                // World Y range for this cell: [cell_min_y, cell_max_y)
                let cell_min_world_y = block_y + (cell_y * v_cell_blocks) as i32;
                let cell_max_world_y = cell_min_world_y + v_cell_blocks as i32;

                // Fast path: if all 8 corners agree on sign, skip interpolation
                match interp.corners_uniform_sign() {
                    Some(false) => {
                        // All non-solid — fill with fluid where world Y < sea_level.
                        if cell_max_world_y <= sea_level {
                            // Entire cell is below sea level: fill all with fluid.
                            let bx_base = cell_x * h_cell_blocks;
                            let by_base = cell_y * v_cell_blocks;
                            let bz_base = cell_z * h_cell_blocks;
                            block_states.fill_box(
                                bx_base,
                                bx_base + h_cell_blocks,
                                by_base,
                                by_base + v_cell_blocks,
                                bz_base,
                                bz_base + h_cell_blocks,
                                default_fluid,
                            );
                            // Per-block interpolation would only re-set the same
                            // fluid (all corners non-solid, every Y below sea level),
                            // so the cell is complete.
                            continue;
                        } else if cell_min_world_y >= sea_level {
                            // Entire cell is at or above sea level: all air.
                            continue;
                        } else {
                            // Cell straddles sea level: fall through to per-block loop.
                        }
                    }
                    Some(true) => {
                        // All solid — fill the entire cell with default block.
                        let bx_base = cell_x * h_cell_blocks;
                        let by_base = cell_y * v_cell_blocks;
                        let bz_base = cell_z * h_cell_blocks;
                        block_states.fill_box(
                            bx_base,
                            bx_base + h_cell_blocks,
                            by_base,
                            by_base + v_cell_blocks,
                            bz_base,
                            bz_base + h_cell_blocks,
                            default_block,
                        );
                        continue;
                    }
                    None => {}
                }

                for local_y in (0..v_cell_blocks).rev() {
                    let delta_y = local_y as f32 / v_cell_blocks as f32;
                    interp.interpolate_y(delta_y);

                    let world_y = block_y + (cell_y * v_cell_blocks + local_y) as i32;

                    for local_x in 0..h_cell_blocks {
                        let delta_x = local_x as f32 / h_cell_blocks as f32;
                        interp.interpolate_x(delta_x);

                        for local_z in 0..h_cell_blocks {
                            let delta_z = local_z as f32 / h_cell_blocks as f32;
                            interp.interpolate_z(delta_z);

                            let density = interp.result();
                            let bx = (cell_x * h_cell_blocks + local_x) as i32;
                            let by = (cell_y * v_cell_blocks + local_y) as i32;
                            let bz = (cell_z * h_cell_blocks + local_z) as i32;

                            if density > 0.0 {
                                block_states.set(BlockPos::new(bx, by, bz), default_block);
                            } else if world_y < sea_level {
                                block_states.set(BlockPos::new(bx, by, bz), default_fluid);
                            }
                        }
                    }
                }
            }
        }

        interp.swap_buffers();
    }

    // Mark section complete so the next section can reuse our top-Y row
    interp.end_section();
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
    section_y: i32,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    column_cache: &ColumnCache,
    biome_source: &BiomeSource,
    biome_registry: &RegistrySnapshot<Biome>,
) {
    let sea_level = noise_router.sea_level();
    for cy in 0..4usize {
        // Center world Y of this biome cell row.
        let cell_center_world_y = section_y * 16 + cy as i32 * 4 + 2;
        let is_ocean = cell_center_world_y < sea_level;
        for cx in 0..4usize {
            let sample_x = block_x + cx as i32 * 4;
            for cz in 0..4usize {
                let sample_z = block_z + cz as i32 * 4;
                let (temp, humidity) = noise_router.sample_climate_at(column_cache, sample_x, sample_z);
                let asset_id = biome_source.beta_biome_id(temp, humidity, is_ocean);
                let network_id = match biome_registry.by_asset_id(asset_id) {
                    Some(id) => id as u8,
                    None => {
                        tracing::error!(?asset_id, "beta biome handle not present in registry snapshot");
                        debug_assert!(false, "unresolved beta biome handle");
                        0
                    }
                };
                biomes.set_cell(cx, cy, cz, network_id);
            }
        }
    }
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
#[cfg_attr(feature = "telemetry-tracy", tracing::instrument(name = "world::column_gen", skip_all))]
pub fn generate_column(
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    cancel: &CancellationToken,
) -> Vec<Option<(BlockPalette, BiomePalette)>> {
    let mut interp = noise_router.new_noise_cell_interpolator();
    let block_x = section_x * 16;
    let block_z = section_z * 16;

    // Pre-populate Zone A values for all 17x17 XZ positions in one pass
    let mut column_cache = noise_router.new_column_cache(block_x, block_z);
    noise_router.populate_columns(&mut column_cache);

    #[cfg(feature = "surface-skip")]
    let skip_above_y = noise_router.estimate_max_surface_y(&column_cache);

    let noise_min_y = noise_router.noise_min_y();
    let noise_max_y = noise_min_y + noise_router.noise_height() as i32;

    // Precompute all corner densities for the column in large batches; the
    // per-section plane fills then copy from this grid.
    {
        let rows = noise_router.noise_height() as usize / interp.v_cell_blocks() + 1;
        interp.precompute_column_grid(noise_router, &mut column_cache, noise_min_y, rows);
    }

    // Only fill biome palettes when the source is Beta; modern paths keep default().
    let beta_biome = biome_context.and_then(|(src, reg)| {
        if matches!(src, BiomeSource::Beta { .. }) {
            Some((src, reg))
        } else {
            None
        }
    });

    let mut prev_sy: Option<i32> = None;
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
            if section_min_y >= noise_max_y || section_max_y <= noise_min_y {
                interp.reset_section_boundary();
                prev_sy = Some(sy);
                let mut biomes = BiomePalette::default();
                if let Some((src, reg)) = beta_biome {
                    fill_biome_palette_beta(&mut biomes, sy, block_x, block_z, noise_router, &column_cache, src, reg);
                }
                return Some((BlockPalette::default(), biomes));
            }

            // Surface skip: sections above estimated max surface are guaranteed all-air
            #[cfg(feature = "surface-skip")]
            if let Some(max_y) = skip_above_y {
                if sy * 16 >= max_y {
                    // Skipped section breaks Y-adjacency, treated as a gap
                    interp.reset_section_boundary();
                    prev_sy = Some(sy);
                    let mut biomes = BiomePalette::default();
                    if let Some((src, reg)) = beta_biome {
                        fill_biome_palette_beta(&mut biomes, sy, block_x, block_z, noise_router, &column_cache, src, reg);
                    }
                    return Some((BlockPalette::default(), biomes));
                }
            }

            // Invalidate Y-boundary cache when sections are not adjacent
            if prev_sy.is_some_and(|prev| prev + 1 != sy) {
                interp.reset_section_boundary();
            }
            prev_sy = Some(sy);

            let mut blocks = BlockPalette::default();
            let mut biomes = BiomePalette::default();
            generate_section(
                block_x,
                sy * 16,
                block_z,
                &mut blocks,
                noise_router,
                &mut column_cache,
                &mut interp,
            );
            if let Some((src, reg)) = beta_biome {
                fill_biome_palette_beta(&mut biomes, sy, block_x, block_z, noise_router, &column_cache, src, reg);
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
    rng: &mut LegacyRandom,
) {
    let Some(beach_noise) = noise_router.beta_beach_noise() else { return };
    let Some(surf_noise) = noise_router.beta_surface_noise() else { return };

    // Extract the quantized biome lookup from the biome source.
    // back2beta's replaceBlocksForBiome reads biomes via getBiomeFromLookup (quantized).
    let beta_lookup = match biome_source {
        BiomeSource::Beta { lookup, .. } => Some(lookup.as_ref()),
        _ => None,
    };

    let sea_level = noise_router.sea_level();
    let default_fluid = noise_router.default_fluid_state();
    let stone = noise_router.default_block_state();
    let bedrock = minecraft::BEDROCK.default_state_id;
    let sandstone = minecraft::SANDSTONE.default_state_id;
    let gravel = minecraft::GRAVEL.default_state_id;

    const D0: f32 = 0.03125;

    // Pre-sample noise arrays for the 16x16 chunk footprint.
    // r[x*16+z]: beach XZ noise (gravel/sand condition).
    // s[x*16+z]: beach noise at Y=109 (gravel override condition).
    // t[x*16+z]: surface depth noise.
    let mut r = [0.0f32; 256];
    let mut s = [0.0f32; 256];
    let mut t = [0.0f32; 256];
    for x in 0..16usize {
        for z in 0..16usize {
            r[x * 16 + z] = beach_noise.sample_xyz_beta(
                (block_x + x as i32) as f32,
                (block_z + z as i32) as f32,
                0.0,
                D0, D0, 1.0,
            );
            s[x * 16 + z] = beach_noise.sample_xyz_beta(
                (block_x + x as i32) as f32,
                109.0134,
                (block_z + z as i32) as f32,
                D0, 1.0, D0,
            );
            t[x * 16 + z] = surf_noise.sample_xyz_beta(
                (block_x + x as i32) as f32,
                (block_z + z as i32) as f32,
                0.0,
                D0 * 2.0, D0 * 2.0, D0 * 2.0,
            );
        }
    }

    // back2beta column loop: outer = x (k=0..16), inner = z (l=0..16).
    // Noise arrays r/s/t are filled at index x*16+z but read by back2beta at
    // r[kk + ll*16] = r[x + z*16]. Reading x+z*16 from an x*16+z fill picks the
    // value stored at fill position (x_fill=z, z_fill=x), i.e., the noise sampled
    // at the transposed world position (block_x+z, block_z+x). Use z*16+x here to
    // replicate back2beta's r[kk+ll*16] read exactly.
    for x_local in 0..16i32 {
        for z_local in 0..16i32 {
            let idx = (z_local * 16 + x_local) as usize;

            // Three RNG draws per column, in back2beta's exact order.
            let flag = r[idx] + rng.next_f64() as f32 * 0.2 > 0.0;
            let flag1 = s[idx] + rng.next_f64() as f32 * 0.2 > 3.0;
            let i1 = (t[idx] / 3.0 + 3.0 + rng.next_f64() as f32 * 0.25) as i32;

            // back2beta's replaceBlocksForBiome reads biome at index (kk + ll*16)
            // where kk=x_local, ll=z_local, but getBiomeArray fills at (x*16 + z).
            // The resulting coordinate transpose means the biome applied to (lx, lz)
            // is sampled at the transposed world position (block_x+lz, block_z+lx).
            let climate_x = block_x + z_local;
            let climate_z = block_z + x_local;
            let (temp, humidity) = noise_router.sample_beta_climate(climate_x, climate_z);
            let biome_land: BetaLandBiome = if let Some(table) = beta_lookup {
                beta_biome_from_climate(table, temp, humidity)
            } else {
                beta_get_biome(temp, humidity)
            };
            let (mut top_block, mut filler_block) = beta_surface_blocks(biome_land);

            // j1 in back2beta: depth counter, -1 means "not yet in surface layer".
            let mut j1: i32 = -1;
            // Mutable top/filler for current Y zone (back2beta: b1, b2).
            let mut b1 = top_block;
            let mut b2 = filler_block;
            let air = mcrs_protocol::BlockStateId(0);

            // Sweep from world Y=127 down to 0 (back2beta: k1 = 127..=0).
            // Bedrock check is interleaved inside this loop.
            for k1 in (0i32..=127).rev() {
                let section_y = k1 >> 4;
                let local_y = k1 & 0xF;
                let si = y_sections.iter().position(|&sy| sy == section_y);

                // Bedrock check (back2beta: k1 <= 0 + this.j.nextInt(5)).
                if k1 <= rng.next_i32_bound(5) {
                    if let Some(si) = si {
                        if let Some((blocks, _)) = sections[si].as_mut() {
                            blocks.set(BlockPos::new(x_local, local_y, z_local), bedrock);
                        }
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
                                    b1 = minecraft::SAND.default_state_id;
                                }
                                if flag {
                                    b2 = minecraft::SAND.default_state_id;
                                }
                            }

                            if k1 < sea_level && b1 == air {
                                b1 = default_fluid;
                            }

                            j1 = i1;
                            if let Some(si) = si {
                                if let Some((blocks, _)) = sections[si].as_mut() {
                                    let place = if k1 >= sea_level - 1 { b1 } else { b2 };
                                    blocks.set(BlockPos::new(x_local, local_y, z_local), place);
                                }
                            }
                        } else if j1 > 0 {
                            j1 -= 1;
                            if let Some(si) = si {
                                if let Some((blocks, _)) = sections[si].as_mut() {
                                    blocks.set(BlockPos::new(x_local, local_y, z_local), b2);
                                }
                            }
                            if j1 == 0 && b2 == minecraft::SAND.default_state_id {
                                j1 = rng.next_i32_bound(4);
                                b2 = sandstone;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
