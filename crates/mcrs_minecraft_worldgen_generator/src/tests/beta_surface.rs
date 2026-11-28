#[cfg(test)]
use crate::ColumnBiomes;
#[cfg(test)]
use mcrs_voxel::LocalPos;

use mcrs_minecraft_biome::source::{BiomeSource, build_beta_lookup_table};
#[cfg(test)]
use mcrs_minecraft_chunk::VoxelId;
#[cfg(test)]
use mcrs_voxel::BlockPos;

#[cfg(test)]
use super::build_beta_router;
#[cfg(test)]
use crate::ColumnBlocks;
#[cfg(test)]
use crate::task::CancellationToken;
#[cfg(test)]
use crate::{apply_beta_surface, generate_column};
use mcrs_minecraft_registries::Registries;
use std::sync::Arc;

#[cfg(test)]
type GeneratedSections = Vec<
    Option<(
        mcrs_minecraft_chunk::palette::BlockPalette,
        mcrs_minecraft_chunk::palette::BiomePalette,
    )>,
>;

pub(crate) fn build_beta_biome_source() -> (BiomeSource, Registries) {
    let land: Vec<String> = (0..11)
        .map(|i| format!("minecraft:land_biome_{i}"))
        .collect();
    let surface = [
        "minecraft:eroded_badlands",
        "minecraft:frozen_ocean",
        "minecraft:deep_frozen_ocean",
    ];
    let names: Vec<&str> = land.iter().map(String::as_str).chain(surface).collect();
    let registry = super::biome_registry(&names);
    let land_biomes = std::array::from_fn(|i| super::biome_id(&registry, &land[i]));
    let biome_source = BiomeSource::Beta {
        land_biomes,
        lookup: Arc::new(build_beta_lookup_table()),
    };
    (biome_source, registry)
}

#[test]
fn beta_surface_bedrock_matches_back2beta_oracle() {
    let router = build_beta_router();
    let (biome_source, _) = build_beta_biome_source();

    let chunk_x = 0i32;
    let chunk_z = 0i32;
    let block_x = chunk_x * 16;
    let block_z = chunk_z * 16;

    let y_sections: Vec<i32> = (0..8).collect();
    let cancel = CancellationToken::new();
    let mut sections = generate_column(
        chunk_x,
        chunk_z,
        &y_sections,
        &router,
        &ColumnBiomes::new(Some(&biome_source), None, None).unwrap(),
        &cancel,
    );

    let mut rng = crate::beta_surface_rng(chunk_x, chunk_z);
    let column = ColumnBlocks::from_sections(&sections, &y_sections);
    apply_beta_surface(
        &column,
        block_x,
        block_z,
        &router,
        &biome_source,
        super::corpus(),
        &mut rng,
    );
    column.write_back(&mut sections);

    let bedrock_id = VoxelId::from(
        super::corpus()
            .default_state("minecraft:bedrock")
            .unwrap()
            .number(),
    );
    let section0 = sections[0].as_ref().expect("section 0 must be present");
    let blocks = &section0.0;

    let oracle: &[(i32, [u8; 5])] = &[
        (0, [7, 1, 7, 7, 1]),
        (1, [7, 7, 1, 7, 1]),
        (2, [7, 1, 1, 1, 1]),
        (3, [7, 7, 1, 1, 1]),
        (4, [7, 1, 1, 7, 7]),
        (5, [7, 7, 1, 7, 1]),
        (6, [7, 7, 1, 1, 1]),
        (7, [7, 1, 1, 1, 1]),
        (8, [7, 7, 1, 7, 7]),
        (9, [7, 7, 1, 1, 7]),
        (10, [7, 7, 7, 1, 1]),
        (11, [7, 1, 1, 1, 1]),
        (12, [7, 7, 1, 1, 1]),
        (13, [7, 7, 7, 1, 1]),
        (14, [7, 7, 1, 1, 1]),
        (15, [7, 1, 1, 1, 1]),
    ];

    let x_local = 0i32;
    let mut failures: Vec<String> = Vec::new();
    for &(z_local, expected) in oracle {
        for y in 0i32..5 {
            let got_bedrock =
                blocks.get(LocalPos::from(BlockPos::new(x_local, y, z_local))) == bedrock_id;
            let expect_bedrock = expected[y as usize] == 7;
            if got_bedrock != expect_bedrock {
                failures.push(format!(
                    "  x={} z={} Y={}: expected {} got {}",
                    x_local,
                    z_local,
                    y,
                    if expect_bedrock {
                        "bedrock(7)"
                    } else {
                        "stone(1)"
                    },
                    if got_bedrock {
                        "bedrock(7)"
                    } else {
                        "stone(1)"
                    },
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "\nbeta_surface_bedrock_matches_back2beta_oracle FAILED (chunk 0,0, x_local=0):\n{}",
        failures.join("\n"),
    );

    let section0_blocks = &sections[0].as_ref().expect("section 0 must be Some").0;
    let y0_all_bedrock = (0..16i32).all(|x| {
        (0..16i32)
            .all(|z| section0_blocks.get(LocalPos::from(BlockPos::new(x, 0, z))) == bedrock_id)
    });
    assert!(y0_all_bedrock, "world Y=0 must be all bedrock");
}

#[test]
fn beta_terrain_height_matches_back2beta_oracle() {
    let corpus = super::beta_surface_capture::load_corpus();

    let corpus_terrain_top_y = |pre_cave: &[u8]| -> Option<i32> {
        (0..128i32).rev().find(|&y| {
            let b = pre_cave[y as usize];
            b != 0 && b != 9
        })
    };

    let stone_id = VoxelId::from(
        super::corpus()
            .default_state("minecraft:stone")
            .unwrap()
            .number(),
    );

    let rust_stone_top_y = |sections: &GeneratedSections, lx: i32, lz: i32| -> Option<i32> {
        let y_sections: Vec<i32> = (0..8).collect();
        for sy in (0..8i32).rev() {
            let si = sy as usize;
            if let Some(Some((blocks, _))) = sections.get(si) {
                let base_y = sy * 16;
                for local_y in (0..16i32).rev() {
                    if blocks.get(LocalPos::from(BlockPos::new(lx, local_y, lz))) == stone_id {
                        return Some(base_y + local_y);
                    }
                }
            }
        }
        let _ = y_sections;
        None
    };

    let worst_offenders: &[(i32, i32)] = &[
        (24, -16),
        (15, -16),
        (22, -16),
        (23, -16),
        (23, -15),
        (24, -13),
    ];

    let router = build_beta_router();
    let (biome_source, _) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let y_sections: Vec<i32> = (0..8).collect();

    let corpus_map: std::collections::HashMap<(i32, i32), &[u8]> = corpus
        .columns
        .iter()
        .map(|c| ((c.wx, c.wz), c.pre_cave.as_slice()))
        .collect();

    let mut chunk_cache: std::collections::HashMap<(i32, i32), GeneratedSections> =
        std::collections::HashMap::new();

    let mut failures: Vec<String> = Vec::new();
    let mut table_rows: Vec<String> = Vec::new();

    for &(wx, wz) in worst_offenders {
        let cx = wx >> 4;
        let cz = wz >> 4;
        let lx = wx - cx * 16;
        let lz = wz - cz * 16;

        let sections = chunk_cache.entry((cx, cz)).or_insert_with(|| {
            generate_column(
                cx,
                cz,
                &y_sections,
                &router,
                &ColumnBiomes::new(Some(&biome_source), None, None).unwrap(),
                &cancel,
            )
        });

        let rust_top = rust_stone_top_y(sections, lx, lz);
        let corpus_top = corpus_map
            .get(&(wx, wz))
            .and_then(|pc| corpus_terrain_top_y(pc));

        let rust_y = rust_top.unwrap_or(-1);
        let corpus_y = corpus_top.unwrap_or(-1);
        let delta = rust_y - corpus_y;

        table_rows.push(format!(
            "  ({:+5},{:+5})  corpus_top={:3}  rust_top={:3}  delta={:+4}",
            wx, wz, corpus_y, rust_y, delta
        ));

        if rust_top != corpus_top {
            failures.push(format!(
                "column ({},{}) rust_stone_top={:?} != corpus_terrain_top={:?}",
                wx, wz, rust_top, corpus_top
            ));
        }
    }

    println!("\n=== Terrain-top Y diagnostic (seed 12345) ===");
    println!("  {:^11}   corpus_top   rust_top   delta", "(wx,wz)");
    for row in &table_rows {
        println!("{}", row);
    }

    assert!(
        failures.is_empty(),
        "\nbeta_terrain_height_matches_back2beta_oracle FAILED:\n\
         The worst-offender stone-top Ys do not match the corpus.\n\
         Per-column results:\n{}\n",
        failures.join("\n")
    );
}
