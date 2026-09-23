use bevy_math::IVec3;
use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;

use crate::site::{Context, Stub};

pub mod buried_treasure;
pub mod desert_pyramid;
pub mod end_city;
pub mod fortress;
pub mod igloo;
pub mod jungle_temple;
pub mod mineshaft;
pub mod nether_fossil;
pub mod ocean_monument;
pub mod ocean_ruin;
pub mod ruined_portal;
pub mod shipwreck;
pub mod stronghold;
pub mod swamp_hut;
pub mod woodland_mansion;

pub(crate) fn random_rotation(rng: &mut LegacyRandom) -> Rotation {
    Rotation::ALL[rng.next_i32_bound(4) as usize]
}

/// `couldValidBiomeExistInTerrainColumn`: one block below the accessor's
/// floor, unlike the jigsaw column test.
pub(crate) fn terrain_column_admits(ctx: &mut Context<'_>, x: i32, z: i32) -> bool {
    let max_y = ctx.accessor_min_y + ctx.accessor_height - 1;
    ctx.world
        .column_admits(x, z, ctx.accessor_min_y - 1, max_y, &ctx.structure.biomes)
}

pub(crate) fn first_occupied_height(
    ctx: &mut Context<'_>,
    x: i32,
    z: i32,
    heightmap: HeightmapName,
) -> i32 {
    ctx.world.free_height(x, z, heightmap) - 1
}

pub(crate) fn on_top_of_chunk_centre(
    ctx: &mut Context<'_>,
    heightmap: HeightmapName,
) -> Option<(IVec3, Stub)> {
    let (x, z) = (ctx.chunk.middle_block_x(), ctx.chunk.middle_block_z());
    terrain_column_admits(ctx, x, z)
        .then(|| on_top_of_chunk_centre_without_biome_check(ctx, heightmap))
}

pub(crate) fn on_top_of_chunk_centre_without_biome_check(
    ctx: &mut Context<'_>,
    heightmap: HeightmapName,
) -> (IVec3, Stub) {
    let (x, z) = (ctx.chunk.middle_block_x(), ctx.chunk.middle_block_z());
    let y = first_occupied_height(ctx, x, z, heightmap);
    (IVec3::new(x, y, z), Stub::Plain)
}

pub(crate) fn corner_heights(
    ctx: &mut Context<'_>,
    min_x: i32,
    size_x: i32,
    min_z: i32,
    size_z: i32,
) -> [i32; 4] {
    [
        (min_x, min_z),
        (min_x, min_z + size_z),
        (min_x + size_x, min_z),
        (min_x + size_x, min_z + size_z),
    ]
    .map(|(x, z)| first_occupied_height(ctx, x, z, HeightmapName::WorldSurfaceWg))
}

pub(crate) fn lowest_y(
    ctx: &mut Context<'_>,
    min_x: i32,
    min_z: i32,
    size_x: i32,
    size_z: i32,
) -> i32 {
    corner_heights(ctx, min_x, size_x, min_z, size_z)
        .into_iter()
        .min()
        .expect("four corners")
}

pub(crate) fn lowest_y_in_5_by_5(ctx: &mut Context<'_>, x: i32, z: i32, rotation: Rotation) -> i32 {
    let (offset_x, offset_z) = match rotation {
        Rotation::None => (5, 5),
        Rotation::Clockwise90 => (-5, 5),
        Rotation::Clockwise180 => (-5, -5),
        Rotation::Counterclockwise90 => (5, -5),
    };
    lowest_y(ctx, x, z, offset_x, offset_z)
}

/// `SinglePieceStructure.findGenerationPoint`.
pub(crate) fn single_piece_site(
    ctx: &mut Context<'_>,
    width: i32,
    depth: i32,
) -> Option<(IVec3, Stub)> {
    let (x, z) = (ctx.chunk.middle_block_x(), ctx.chunk.middle_block_z());
    if !terrain_column_admits(ctx, x, z) {
        return None;
    }
    let (min_x, min_z) = (ctx.chunk.min_block_x(), ctx.chunk.min_block_z());
    if lowest_y(ctx, min_x, min_z, width, depth) < ctx.height.sea_level {
        return None;
    }
    Some(on_top_of_chunk_centre_without_biome_check(
        ctx,
        HeightmapName::WorldSurfaceWg,
    ))
}

/// The end city's and the mansion's site: the biome column at block 7 of the
/// chunk, a rotation, and the lowest corner of the 5×5 box that rotation
/// points into, refused below 60.
pub(crate) fn lowest_corner_site(
    ctx: &mut Context<'_>,
    rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    let (x, z) = (ctx.chunk.min_block_x() + 7, ctx.chunk.min_block_z() + 7);
    if !terrain_column_admits(ctx, x, z) {
        return None;
    }
    let rotation = random_rotation(rng);
    let y = lowest_y_in_5_by_5(ctx, x, z, rotation);
    (y >= 60).then_some((IVec3::new(x, y, z), Stub::Rotated(rotation)))
}
