use crate::ColumnBiomes;
use mcrs_voxel::LocalPos;
use std::collections::BTreeMap;

use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_carver::beta::carve_beta_caves;
use mcrs_minecraft_worldgen_carver::mask::CarvingMask;
use mcrs_minecraft_worldgen_carver::modern::SOURCE_RADIUS;
use mcrs_minecraft_worldgen_carver::water::WaterMask;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use super::beta_surface::build_beta_biome_source;
use super::beta_surface_capture::{ColumnFixture, load_corpus};
use super::build_beta_router;
use super::counting_rng::CountingRng;
use crate::ColumnBlocks;
use crate::modern_carvers::CarverBiomeTable;
use crate::stages::extent;
use crate::task::CancellationToken;
use crate::{BetaCaveBlockIds, apply_beta_carvers, apply_beta_surface, generate_column};

fn modern_id_for_beta(beta_id: u8) -> BlockStateId {
    match beta_id {
        0 => super::corpus().default_state("minecraft:air").unwrap(),
        1 => super::corpus().default_state("minecraft:stone").unwrap(),
        2 => super::corpus()
            .default_state("minecraft:grass_block")
            .unwrap(),
        3 => super::corpus().default_state("minecraft:dirt").unwrap(),
        7 => super::corpus().default_state("minecraft:bedrock").unwrap(),
        9 => super::corpus().default_state("minecraft:water").unwrap(),
        12 => super::corpus().default_state("minecraft:sand").unwrap(),
        13 => super::corpus().default_state("minecraft:gravel").unwrap(),
        24 => super::corpus()
            .default_state("minecraft:sandstone")
            .unwrap(),
        79 => super::corpus().default_state("minecraft:ice").unwrap(),
        _ => super::corpus().default_state("minecraft:air").unwrap(),
    }
}

fn beta_id_for_modern(modern: BlockStateId) -> u8 {
    let air = super::corpus().default_state("minecraft:air").unwrap();
    let stone = super::corpus().default_state("minecraft:stone").unwrap();
    let grass = super::corpus()
        .default_state("minecraft:grass_block")
        .unwrap();
    let dirt = super::corpus().default_state("minecraft:dirt").unwrap();
    let bedrock = super::corpus().default_state("minecraft:bedrock").unwrap();
    let sand = super::corpus().default_state("minecraft:sand").unwrap();
    let gravel = super::corpus().default_state("minecraft:gravel").unwrap();
    let sandstone = super::corpus()
        .default_state("minecraft:sandstone")
        .unwrap();
    let water = super::corpus().default_state("minecraft:water").unwrap();
    let lava = super::corpus().default_state("minecraft:lava").unwrap();
    let ice = super::corpus().default_state("minecraft:ice").unwrap();

    if modern == air {
        return 0;
    }
    if modern == stone {
        return 1;
    }
    if modern == grass {
        return 2;
    }
    if modern == dirt {
        return 3;
    }
    if modern == bedrock {
        return 7;
    }
    if modern == water {
        return 9;
    }
    if modern == lava {
        return 10;
    }
    if modern == sand {
        return 12;
    }
    if modern == gravel {
        return 13;
    }
    if modern == sandstone {
        return 24;
    }
    if modern == ice {
        return 79;
    }
    0
}

fn carve(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    router: &NoiseRouter,
    table: &CarverBiomeTable,
) {
    apply_beta_carvers(
        column,
        chunk_x,
        chunk_z,
        world_seed,
        router,
        &mut Workspace::new(),
        table,
        extent(router),
        &BetaCaveBlockIds::resolve(super::corpus()),
    );
}

fn count_rng_draws_for_chunk(chunk_x: i32, chunk_z: i32, world_seed: i64) -> u64 {
    let draws = std::rc::Rc::new(std::cell::Cell::new(0u64));

    let mut seed_rng = LegacyRandom::new(world_seed as u64);
    let l: i64 = seed_rng.next_i64() / 2 * 2 + 1;
    let i1: i64 = seed_rng.next_i64() / 2 * 2 + 1;
    draws.set(draws.get() + 2);

    let water = WaterMask::default();
    let mut mask = CarvingMask::new(1, 120);
    let radius = SOURCE_RADIUS;

    for origin_x in (chunk_x - radius)..=(chunk_x + radius) {
        for origin_z in (chunk_z - radius)..=(chunk_z + radius) {
            let seed: i64 = (origin_x as i64)
                .wrapping_mul(l)
                .wrapping_add((origin_z as i64).wrapping_mul(i1))
                ^ world_seed;

            let mut counting_rng = CountingRng::new(seed as u64, draws.clone());

            carve_beta_caves(
                chunk_x,
                chunk_z,
                origin_x,
                origin_z,
                &water,
                &mut mask,
                &mut counting_rng,
            );
        }
    }

    draws.get()
}

const DRAW_COUNT_CHUNK_0_0_SEED_12345: u64 = 1883;

mod exhaustive {
    use super::*;
    use mcrs_minecraft_biome::source::BiomeSource;

    struct BetaPipeline {
        router: NoiseRouter,
        biome_source: BiomeSource,
        table: CarverBiomeTable,
        y_sections: Vec<i32>,
        cancel: CancellationToken,
    }

    impl BetaPipeline {
        fn new() -> Self {
            let (biome_source, _) = build_beta_biome_source();
            Self {
                router: build_beta_router(),
                table: crate::tests::beta_carver_table(&biome_source),
                biome_source,
                y_sections: (0..8).collect(),
                cancel: CancellationToken::new(),
            }
        }

        fn carved_column(
            &self,
            chunk_x: i32,
            chunk_z: i32,
        ) -> Vec<Option<(BlockPalette, BiomePalette)>> {
            let mut sections = generate_column(
                chunk_x,
                chunk_z,
                &self.y_sections,
                &self.router,
                &ColumnBiomes::new(Some(&self.biome_source), None, None).unwrap(),
                &self.cancel,
            );
            let mut rng = crate::beta_surface_rng(chunk_x, chunk_z);
            let column = ColumnBlocks::from_sections(&sections, &self.y_sections);
            apply_beta_surface(
                &column,
                chunk_x * 16,
                chunk_z * 16,
                &self.router,
                &self.biome_source,
                crate::tests::corpus(),
                &mut rng,
            );
            column.write_back(&mut sections);
            let column = ColumnBlocks::from_sections(&sections, &self.y_sections);
            carve(
                &column,
                chunk_x,
                chunk_z,
                self.router.world_seed as i64,
                &self.router,
                &self.table,
            );
            column.write_back(&mut sections);
            sections
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one parity gate keeps setup and comparison together"
    )]
    #[expect(clippy::excessive_nesting, reason = "a mismatch names its position")]
    fn beta_cave_parity_gate() {
        let corpus = load_corpus();
        let router = build_beta_router();
        let (source, _) = build_beta_biome_source();
        let table = crate::tests::beta_carver_table(&source);

        let world_seed: i64 = 12345;
        let y_sections: Vec<i32> = (0..8).collect();

        let mut chunks: BTreeMap<(i32, i32), Vec<&ColumnFixture>> = BTreeMap::new();
        for col in &corpus.columns {
            let cx = col.wx >> 4;
            let cz = col.wz >> 4;
            chunks.entry((cx, cz)).or_default().push(col);
        }

        let mut total_columns: u64 = 0;
        type ColumnMismatches = Vec<(i32, u8, u8)>;
        let mut mismatches: Vec<(i32, i32, ColumnMismatches)> = Vec::new();

        for ((cx, cz), fixture_cols) in &chunks {
            let block_x = cx * 16;
            let block_z = cz * 16;

            let mut sections: Vec<Option<(BlockPalette, BiomePalette)>> = (0..y_sections.len())
                .map(|_| Some((BlockPalette::default(), BiomePalette::default())))
                .collect();

            for fix_col in fixture_cols.iter() {
                let local_x = fix_col.wx - block_x;
                let local_z = fix_col.wz - block_z;
                for (si, &sy) in y_sections.iter().enumerate() {
                    let base_y = sy * 16;
                    if let Some(Some((palette, _))) = sections.get_mut(si) {
                        for local_y in 0..16i32 {
                            let world_y = base_y + local_y;
                            if world_y < 128 {
                                let beta_id = fix_col.pre_cave[world_y as usize];
                                palette.set(
                                    LocalPos::from(BlockPos::new(local_x, local_y, local_z)),
                                    modern_id_for_beta(beta_id).number().into(),
                                );
                            }
                        }
                    }
                }
            }

            let column = ColumnBlocks::from_sections(&sections, &y_sections);
            carve(&column, *cx, *cz, world_seed, &router, &table);
            column.write_back(&mut sections);

            for fix_col in fixture_cols.iter() {
                total_columns += 1;
                let local_x = fix_col.wx - block_x;
                let local_z = fix_col.wz - block_z;

                let mut col_mismatches = ColumnMismatches::new();
                for (si, &sy) in y_sections.iter().enumerate() {
                    if let Some(Some((palette, _))) = sections.get(si) {
                        let base_y = sy * 16;
                        for local_y in 0..16i32 {
                            let world_y = base_y + local_y;
                            if world_y < 128 {
                                let state = palette
                                    .get(LocalPos::from(BlockPos::new(local_x, local_y, local_z)));
                                let got = beta_id_for_modern(crate::tests::corpus_state_of(state));
                                let want = fix_col.post_cave[world_y as usize];
                                if got != want {
                                    col_mismatches.push((world_y, got, want));
                                }
                            }
                        }
                    }
                }
                if !col_mismatches.is_empty() {
                    mismatches.push((fix_col.wx, fix_col.wz, col_mismatches));
                }
            }
        }

        if !mismatches.is_empty() {
            let first_10: Vec<String> = mismatches
                .iter()
                .take(10)
                .map(|(wx, wz, diffs)| {
                    let first_diff = diffs
                        .first()
                        .map(|(y, got, want)| format!("Y={} got={} want={}", y, got, want))
                        .unwrap_or_default();
                    format!(
                        "  ({:+5},{:+5}): {} block mismatches [{}]",
                        wx,
                        wz,
                        diffs.len(),
                        first_diff
                    )
                })
                .collect();

            panic!(
                "\nBETA CAVE PARITY GATE FAILED\n\
             Columns: {} total, {} mismatched\n\
             First offenders:\n{}\n",
                total_columns,
                mismatches.len(),
                first_10.join("\n"),
            );
        }

        assert_eq!(
            mismatches.len(),
            0,
            "cave parity: {} mismatches / {} columns",
            mismatches.len(),
            total_columns
        );
    }

    #[test]
    fn beta_cave_draw_count_pin() {
        let count = count_rng_draws_for_chunk(0, 0, 12345);

        if DRAW_COUNT_CHUNK_0_0_SEED_12345 == 0 {
            println!("DRAW COUNT PIN (chunk 0,0 seed 12345): {}", count);
            assert!(count > 0, "carver must consume RNG draws");
        } else {
            assert_eq!(
                count, DRAW_COUNT_CHUNK_0_0_SEED_12345,
                "LegacyRandom draw count for chunk (0,0) at seed 12345 changed: expected {}, got {}",
                DRAW_COUNT_CHUNK_0_0_SEED_12345, count,
            );
        }
    }

    #[test]
    #[expect(clippy::excessive_nesting, reason = "per-cell test inline in one scan")]
    fn generate_column_beta_has_caves() {
        let pipeline = BetaPipeline::new();
        let y_sections = &pipeline.y_sections;
        let ids = BetaCaveBlockIds::resolve(crate::tests::corpus());
        let sections = pipeline.carved_column(0, 0);

        let air = ids.air;
        let lava = ids.lava;

        let mut found_cave_air = false;
        let mut found_lava_below_10 = false;

        for local_x in 0..16i32 {
            for local_z in 0..16i32 {
                let mut surface_y = 0i32;
                'surface: for si in (0..y_sections.len()).rev() {
                    let sy = y_sections[si];
                    if let Some(Some((blocks, _))) = sections.get(si) {
                        let base_y = sy * 16;
                        for local_y in (0..16i32).rev() {
                            if blocks.get(LocalPos::from(BlockPos::new(local_x, local_y, local_z)))
                                != air
                            {
                                surface_y = base_y + local_y;
                                break 'surface;
                            }
                        }
                    }
                }

                for (si, &sy) in y_sections.iter().enumerate() {
                    if let Some(Some((blocks, _))) = sections.get(si) {
                        let base_y = sy * 16;
                        for local_y in 0..16i32 {
                            let world_y = base_y + local_y;
                            if world_y >= surface_y {
                                continue;
                            }
                            let state = blocks
                                .get(LocalPos::from(BlockPos::new(local_x, local_y, local_z)));
                            if state == air {
                                found_cave_air = true;
                            }
                            if state == lava && world_y <= 10 {
                                found_lava_below_10 = true;
                            }
                        }
                    }
                }
            }
        }

        assert!(
            found_cave_air,
            "chunk (0,0) at seed 12345 must contain cave air below the surface"
        );
        assert!(
            found_lava_below_10,
            "chunk (0,0) at seed 12345 must contain lava at/below Y 10 from cave carving"
        );
    }

    #[test]
    #[expect(clippy::excessive_nesting, reason = "per-cell test inline in one scan")]
    fn beta_real_pipeline_has_cave_air_below_y32() {
        let pipeline = BetaPipeline::new();
        let y_sections = &pipeline.y_sections;
        let air = BetaCaveBlockIds::resolve(crate::tests::corpus()).air;

        let mut chunks_with_air = 0usize;
        let mut total_air = 0usize;

        for chunk_x in 0..8i32 {
            for chunk_z in 0..8i32 {
                let sections = pipeline.carved_column(chunk_x, chunk_z);

                let mut air_below_32 = 0usize;
                for (si, &sy) in y_sections.iter().enumerate() {
                    let base_y = sy * 16;
                    if base_y >= 32 {
                        continue;
                    }
                    if let Some(Some((blocks, _))) = sections.get(si) {
                        for local_x in 0..16i32 {
                            for local_z in 0..16i32 {
                                for local_y in 0..16i32 {
                                    if base_y + local_y >= 32 {
                                        continue;
                                    }
                                    if blocks.get(LocalPos::from(BlockPos::new(
                                        local_x, local_y, local_z,
                                    ))) == air
                                    {
                                        air_below_32 += 1;
                                    }
                                }
                            }
                        }
                    }
                }

                if air_below_32 > 0 {
                    chunks_with_air += 1;
                    total_air += air_below_32;
                    eprintln!("chunk ({chunk_x},{chunk_z}): {air_below_32} air voxels below Y 32");
                }
            }
        }

        eprintln!(
            "REAL PIPELINE seed 12345: {chunks_with_air}/64 chunks have cave air below Y 32; {total_air} air voxels total"
        );

        assert!(
            chunks_with_air >= 1,
            "real Beta generation pipeline must produce at least 1 chunk with air below Y 32"
        );
    }
}
