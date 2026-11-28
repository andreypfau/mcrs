use crate::ColumnBiomes;
use mcrs_voxel::LocalPos;
use std::collections::BTreeMap;

use mcrs_minecraft_biome::source::{
    BetaLandBiome, beta_biome_from_climate, build_beta_lookup_table,
};
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use mcrs_minecraft_worldgen_density::program::Workspace;

use super::beta_surface::build_beta_biome_source;
use super::build_beta_router;
use crate::ColumnBlocks;
use crate::task::CancellationToken;
use crate::{apply_beta_surface, generate_column};

#[derive(serde::Deserialize)]
pub(super) struct BetaSurfaceCorpus {
    pub(super) columns: Vec<ColumnFixture>,
}

#[derive(serde::Deserialize)]
pub(super) struct ColumnFixture {
    pub(super) wx: i32,
    pub(super) wz: i32,
    biome_id: u8,
    #[serde(with = "serde_base64")]
    pub(super) pre_cave: Vec<u8>,
    #[serde(with = "serde_base64")]
    pub(super) post_cave: Vec<u8>,
}

mod serde_base64 {
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(de)?;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)
    }
}

pub(super) fn load_corpus() -> BetaSurfaceCorpus {
    serde_json::from_str(include_str!("fixtures/beta_surface_corpus.json"))
        .expect("valid corpus JSON")
}

fn beta_id_for(modern: BlockStateId) -> u8 {
    let air = super::corpus().default_state("minecraft:air").unwrap();
    let stone = super::corpus().default_state("minecraft:stone").unwrap();
    let grass = super::corpus()
        .default_state("minecraft:grass_block")
        .unwrap();
    let dirt = super::corpus().default_state("minecraft:dirt").unwrap();
    let bedrock = super::corpus().default_state("minecraft:bedrock").unwrap();
    let sand = super::corpus().default_state("minecraft:sand").unwrap();
    let sandstone = super::corpus()
        .default_state("minecraft:sandstone")
        .unwrap();
    let gravel = super::corpus().default_state("minecraft:gravel").unwrap();

    let water_source = super::corpus().default_state("minecraft:water").unwrap();
    let lava_source = super::corpus().default_state("minecraft:lava").unwrap();

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
    if modern == water_source {
        return 9;
    }
    if modern == lava_source {
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
    let ice = super::corpus().default_state("minecraft:ice").unwrap();
    if modern == ice {
        return 79;
    }

    0
}

fn beta_land_biome_to_back2beta_id(b: BetaLandBiome) -> u8 {
    match b {
        BetaLandBiome::Rainforest => 0,
        BetaLandBiome::Swampland => 1,
        BetaLandBiome::SeasonalForest => 2,
        BetaLandBiome::Forest => 3,
        BetaLandBiome::Savanna => 4,
        BetaLandBiome::Shrubland => 5,
        BetaLandBiome::Taiga => 6,
        BetaLandBiome::Desert => 7,
        BetaLandBiome::Plains => 8,
        BetaLandBiome::IceDesert => 9,
        BetaLandBiome::Tundra => 10,
    }
}

const SURFACE_BAND_HALF: i32 = 8;

#[derive(Debug)]
enum ColumnMatchResult {
    Match,
    Mismatch(ColumnMismatch),
}

#[derive(Debug)]
struct ColumnMismatch {
    wx: i32,
    wz: i32,
    block_mismatches: Vec<(i32, u8, u8, MismatchBand)>,
    biome_mismatch: Option<(u8, u8)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MismatchBand {
    Surface,
    Bedrock,
}

impl std::fmt::Display for MismatchBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MismatchBand::Surface => write!(f, "surface"),
            MismatchBand::Bedrock => write!(f, "bedrock"),
        }
    }
}

fn column_matches(
    generated_col: &[BlockStateId; 128],
    fixture: &ColumnFixture,
    router: &mcrs_minecraft_worldgen_density::router::NoiseRouter,
    table: &[[BetaLandBiome; 64]; 64],
) -> ColumnMatchResult {
    let pre_cave = &fixture.pre_cave;

    let surface_y: i32 = (0..128i32)
        .rev()
        .find(|&y| {
            generated_col[y as usize] != super::corpus().default_state("minecraft:air").unwrap()
        })
        .unwrap_or(0);

    let surface_lo = (surface_y - SURFACE_BAND_HALF).max(0);
    let surface_hi = (surface_y + SURFACE_BAND_HALF).min(127);

    let sea_lo = 60i32;
    let sea_hi = 64i32;

    let mut block_mismatches: Vec<(i32, u8, u8, MismatchBand)> = Vec::new();

    for y in surface_lo..=surface_hi {
        let gen_id = beta_id_for(generated_col[y as usize]);
        let fix_id = pre_cave[y as usize];
        if gen_id != fix_id {
            block_mismatches.push((y, gen_id, fix_id, MismatchBand::Surface));
        }
    }

    if sea_lo < surface_lo || sea_hi > surface_hi {
        for y in sea_lo..=sea_hi {
            if y >= surface_lo && y <= surface_hi {
                continue;
            }
            let gen_id = beta_id_for(generated_col[y as usize]);
            let fix_id = pre_cave[y as usize];
            if gen_id != fix_id {
                block_mismatches.push((y, gen_id, fix_id, MismatchBand::Surface));
            }
        }
    }

    for y in 0i32..=4 {
        let gen_id = beta_id_for(generated_col[y as usize]);
        let fix_id = pre_cave[y as usize];
        if gen_id != fix_id {
            block_mismatches.push((y, gen_id, fix_id, MismatchBand::Bedrock));
        }
    }

    let (temp, humidity) =
        router.sample_beta_climate(&mut Workspace::new(), fixture.wx, fixture.wz);
    let gen_biome = beta_biome_from_climate(table, temp, humidity);
    let gen_back2beta_id = beta_land_biome_to_back2beta_id(gen_biome);
    let biome_mismatch = if gen_back2beta_id != fixture.biome_id {
        Some((gen_back2beta_id, fixture.biome_id))
    } else {
        None
    };

    if block_mismatches.is_empty() && biome_mismatch.is_none() {
        ColumnMatchResult::Match
    } else {
        ColumnMatchResult::Mismatch(ColumnMismatch {
            wx: fixture.wx,
            wz: fixture.wz,
            block_mismatches,
            biome_mismatch,
        })
    }
}

mod exhaustive {
    use super::*;

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one parity gate classifies each mismatch by cause"
    )]
    #[expect(
        clippy::excessive_nesting,
        reason = "a parity miss names the column and the height"
    )]
    fn beta_surface_parity_gate() {
        let corpus = load_corpus();
        let router = build_beta_router();
        let (biome_source, _) = build_beta_biome_source();
        let cancel = CancellationToken::new();
        let biome_table = build_beta_lookup_table();

        let y_sections: Vec<i32> = (0..8).collect();

        let mut chunks: BTreeMap<(i32, i32), Vec<&ColumnFixture>> = BTreeMap::new();
        for col in &corpus.columns {
            let cx = col.wx >> 4;
            let cz = col.wz >> 4;
            chunks.entry((cx, cz)).or_default().push(col);
        }

        let mut total_columns: u64 = 0;
        let mut all_mismatches: Vec<ColumnMismatch> = Vec::new();

        for ((cx, cz), fixture_cols) in &chunks {
            let block_x = cx * 16;
            let block_z = cz * 16;

            let mut sections = generate_column(
                *cx,
                *cz,
                &y_sections,
                &router,
                &ColumnBiomes::new(Some(&biome_source), None, None).unwrap(),
                &cancel,
            );

            let mut rng = crate::beta_surface_rng(*cx, *cz);
            let column = ColumnBlocks::from_sections(&sections, &y_sections);
            apply_beta_surface(
                &column,
                block_x,
                block_z,
                &router,
                &biome_source,
                crate::tests::corpus(),
                &mut rng,
            );
            column.write_back(&mut sections);

            for fix_col in fixture_cols {
                total_columns += 1;

                let local_x = fix_col.wx - block_x;
                let local_z = fix_col.wz - block_z;

                let mut generated: [BlockStateId; 128] = [BlockStateId::AIR; 128];
                for (si, sy) in y_sections.iter().enumerate() {
                    if let Some(Some((blocks, _))) = sections.get(si) {
                        let base_y = sy * 16;
                        for local_y in 0..16i32 {
                            let world_y = base_y + local_y;
                            if world_y < 128 {
                                generated[world_y as usize] =
                                    crate::tests::corpus_state_of(blocks.get(LocalPos::from(
                                        BlockPos::new(local_x, local_y, local_z),
                                    )));
                            }
                        }
                    }
                }

                match column_matches(&generated, fix_col, &router, &biome_table) {
                    ColumnMatchResult::Match => {}
                    ColumnMatchResult::Mismatch(m) => all_mismatches.push(m),
                }
            }
        }

        let mismatched = all_mismatches.len() as u64;

        if mismatched * 1280 > total_columns {
            let bedrock_count = all_mismatches
                .iter()
                .filter(|m| {
                    m.block_mismatches
                        .iter()
                        .any(|b| b.3 == MismatchBand::Bedrock)
                })
                .count();
            let biome_count = all_mismatches
                .iter()
                .filter(|m| m.biome_mismatch.is_some())
                .count();
            let surface_only_count = all_mismatches
                .iter()
                .filter(|m| {
                    m.biome_mismatch.is_none()
                        && m.block_mismatches
                            .iter()
                            .all(|b| b.3 == MismatchBand::Surface)
                })
                .count();

            let mut sorted = all_mismatches.iter().collect::<Vec<_>>();
            sorted.sort_by_key(|m| std::cmp::Reverse(m.block_mismatches.len()));
            let worst: Vec<String> = sorted
                .iter()
                .take(10)
                .map(|m| {
                    let first_block = m
                        .block_mismatches
                        .first()
                        .map(|(y, gid, fid, band)| {
                            format!("Y={} band={} gen={} fix={}", y, band, gid, fid)
                        })
                        .unwrap_or_default();
                    let biome_s = m
                        .biome_mismatch
                        .map(|(g, f)| format!(", biome gen={} fix={}", g, f))
                        .unwrap_or_default();
                    format!(
                        "  ({:+5},{:+5}) {} block mismatches [{}{}]",
                        m.wx,
                        m.wz,
                        m.block_mismatches.len(),
                        first_block,
                        biome_s
                    )
                })
                .collect();

            let cause_note = if bedrock_count > 0 {
                format!(
                    "LIKELY CAUSE: RNG-order or surface-pass bug ({} bedrock-band mismatches — \
                 bedrock is placed by the same LegacyRandom stream as the surface; \
                 wrong draw order corrupts both)",
                    bedrock_count
                )
            } else if biome_count > 0 {
                format!(
                    "LIKELY CAUSE: Climate-sampling or biome-algorithm divergence \
                 ({} biome mismatches)",
                    biome_count
                )
            } else {
                format!(
                    "LIKELY CAUSE: Genuine f32 zero-crossing drift ({} surface-band-only \
                 mismatches, no bedrock or biome failures — sparse f32 accumulation error \
                 expected near terrain boundary; consider widening band or f64 path \
                 after user review)",
                    surface_only_count
                )
            };

            panic!(
                "\n\
             ╔══════════════════════════════════════════════════════════════╗\n\
             ║          BETA SURFACE PARITY GATE FAILED                    ║\n\
             ╚══════════════════════════════════════════════════════════════╝\n\
             \n\
             Threshold : at most 1 mismatch per 1280 columns (~99.9%)\n\
             Seed      : 12345\n\
             Columns   : {} total, {} mismatched ({:.4}%)\n\
             \n\
             Band breakdown:\n\
               Bedrock-band mismatches (Y 0-4)  : {}\n\
               Surface-band only mismatches     : {}\n\
               Biome mismatches                 : {}\n\
             \n\
             {}\n\
             \n\
             Worst-offender columns (most block mismatches first):\n\
             {}\n\
             \n\
             NOTE: Do NOT relax the threshold or enable f64 fallback without \
             explicit user review.  This gate halts the phase intentionally.\n",
                total_columns,
                mismatched,
                mismatched as f64 / total_columns as f64 * 100.0,
                bedrock_count,
                surface_only_count,
                biome_count,
                cause_note,
                worst.join("\n"),
            );
        }

        assert!(
            mismatched * 1280 <= total_columns,
            "parity gate: {} mismatches / {} columns exceeds 1-per-1280 threshold",
            mismatched,
            total_columns
        );
    }

    #[test]
    fn beta_climate_matches_back2beta_oracle() {
        let router = build_beta_router();

        let oracle_cols: &[(i32, i32, u8)] = &[(31, -9, 7), (30, -11, 7), (0, 0, 7), (16, 16, 4)];

        let table = build_beta_lookup_table();

        let mut failures: Vec<String> = Vec::new();

        for &(wx, wz, expected_biome_id) in oracle_cols {
            let (temp, rain) = router.sample_beta_climate(&mut Workspace::new(), wx, wz);
            let gen_id =
                beta_land_biome_to_back2beta_id(beta_biome_from_climate(&table, temp, rain));

            if gen_id != expected_biome_id {
                failures.push(format!(
                    "  ({:+4},{:+4}): expected biome_id={} got {} (temp={:.6} rain={:.6})",
                    wx, wz, expected_biome_id, gen_id, temp, rain
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "\nbeta_climate_matches_back2beta_oracle FAILED;\nfailing columns:\n{}",
            failures.join("\n")
        );
    }
}
