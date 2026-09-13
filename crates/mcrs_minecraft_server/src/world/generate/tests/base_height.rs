use mcrs_minecraft_worldgen::program::Workspace;

use crate::world::chunk::CancellationToken;
use crate::world::generate::{ColumnBlocks, base_height, fill_column_dense_any};
use crate::world::heightmap::{HeightmapKinds, build_pre_carve_heightmaps, heightmap_predicates};

use super::{block_tags, blocks, build_settings_router};

const MIN_Y: i32 = -64;
const HEIGHT: i32 = 384;
const COLUMNS: [(i32, i32); 6] = [(0, 0), (15, 15), (7, 8), (3, 12), (15, 0), (0, 15)];

/// The column sampled alone answers what the full chunk fill's pre-carve maps
/// answer, on land, on the coast and over deep ocean.
#[test]
fn base_height_matches_the_pre_carve_heightmaps() {
    let router = build_settings_router("overworld", 42);
    let predicates = heightmap_predicates(blocks(), block_tags());
    let y_sections: Vec<i32> = (-4..20).collect();
    let mut column = ColumnBlocks::new(&y_sections);
    let mut ws = Workspace::new();
    for (chunk_x, chunk_z) in [(3, -7), (-12, 5), (0, 0), (40, 0)] {
        fill_column_dense_any(
            &mut column,
            chunk_x,
            chunk_z,
            &y_sections,
            &router,
            None,
            None,
            &CancellationToken::new(),
        )
        .expect("the column is not cancelled");
        let maps = build_pre_carve_heightmaps(&column, &predicates).expect("a dense column");
        for (lx, lz) in COLUMNS {
            let (x, z) = (chunk_x * 16 + lx, chunk_z * 16 + lz);
            let surface = base_height(
                &router,
                &mut ws,
                &predicates,
                HeightmapKinds::SURFACE,
                x,
                z,
                MIN_Y,
                HEIGHT,
            );
            let solid = base_height(
                &router,
                &mut ws,
                &predicates,
                HeightmapKinds::SOLID,
                x,
                z,
                MIN_Y,
                HEIGHT,
            );
            assert_eq!(
                surface,
                maps.surface.get(lx as usize, lz as usize),
                "WORLD_SURFACE_WG at ({x}, {z})"
            );
            assert_eq!(
                solid,
                maps.solid.get(lx as usize, lz as usize),
                "OCEAN_FLOOR_WG at ({x}, {z})"
            );
        }
    }
}

#[test]
fn deep_ocean_surface_is_the_water_and_the_floor_lies_below_it() {
    let router = build_settings_router("overworld", 42);
    let predicates = heightmap_predicates(blocks(), block_tags());
    let mut ws = Workspace::new();
    let (x, z) = (40 * 16 + 7, 8);
    let surface = base_height(
        &router,
        &mut ws,
        &predicates,
        HeightmapKinds::SURFACE,
        x,
        z,
        MIN_Y,
        HEIGHT,
    );
    let solid = base_height(
        &router,
        &mut ws,
        &predicates,
        HeightmapKinds::SOLID,
        x,
        z,
        MIN_Y,
        HEIGHT,
    );
    assert_eq!(surface, router.sea_level);
    assert!(
        solid < surface,
        "floor {solid} under the water surface {surface}"
    );
    assert!(solid > MIN_Y, "the floor is a real block, not the fallback");
}

#[test]
fn an_accessor_outside_the_noise_range_answers_its_floor() {
    let router = build_settings_router("overworld", 42);
    let predicates = heightmap_predicates(blocks(), block_tags());
    let mut ws = Workspace::new();
    let height = base_height(
        &router,
        &mut ws,
        &predicates,
        HeightmapKinds::SURFACE,
        0,
        0,
        400,
        16,
    );
    assert_eq!(height, 400);
}
