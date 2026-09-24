use bevy_math::IVec3;
use mcrs_minecraft_core::Mirror;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::template::bounding_box;

use super::{corner_heights, lowest_y, on_top_of_chunk_centre};
use crate::frozen::TemplateId;
use crate::orient::random_rotation;
use crate::piece::{Piece, ShipwreckPiece};
use crate::site::{Context, Site, Stub};

pub const BEACHED: &[&str] = &[
    "shipwreck/with_mast",
    "shipwreck/sideways_full",
    "shipwreck/sideways_fronthalf",
    "shipwreck/sideways_backhalf",
    "shipwreck/rightsideup_full",
    "shipwreck/rightsideup_fronthalf",
    "shipwreck/rightsideup_backhalf",
    "shipwreck/with_mast_degraded",
    "shipwreck/rightsideup_full_degraded",
    "shipwreck/rightsideup_fronthalf_degraded",
    "shipwreck/rightsideup_backhalf_degraded",
];

pub const OCEAN: &[&str] = &[
    "shipwreck/with_mast",
    "shipwreck/upsidedown_full",
    "shipwreck/upsidedown_fronthalf",
    "shipwreck/upsidedown_backhalf",
    "shipwreck/sideways_full",
    "shipwreck/sideways_fronthalf",
    "shipwreck/sideways_backhalf",
    "shipwreck/rightsideup_full",
    "shipwreck/rightsideup_fronthalf",
    "shipwreck/rightsideup_backhalf",
    "shipwreck/with_mast_degraded",
    "shipwreck/upsidedown_full_degraded",
    "shipwreck/upsidedown_fronthalf_degraded",
    "shipwreck/upsidedown_backhalf_degraded",
    "shipwreck/sideways_full_degraded",
    "shipwreck/sideways_fronthalf_degraded",
    "shipwreck/sideways_backhalf_degraded",
    "shipwreck/rightsideup_full_degraded",
    "shipwreck/rightsideup_fronthalf_degraded",
    "shipwreck/rightsideup_backhalf_degraded",
];

/// The beached list is a subset of the ocean one.
pub const TEMPLATES: &[&str] = OCEAN;

const REGION_FIT: i32 = 32;

pub fn heightmap(is_beached: bool) -> HeightmapName {
    if is_beached {
        HeightmapName::WorldSurfaceWg
    } else {
        HeightmapName::OceanFloorWg
    }
}

pub fn site(
    is_beached: bool,
    ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    on_top_of_chunk_centre(ctx, heightmap(is_beached))
}

/// The reference lowers the piece at placement from the live heightmaps of
/// whichever chunk decorates it first: a piece that fits its region reads its
/// whole unrotated footprint, a larger one the four corners of its box; a
/// beached piece then spends a draw of that chunk's placement stream. Here
/// both read the density heights at layout and the draw ends the layout
/// stream, so every column agrees on the height.
pub fn layout(
    is_beached: bool,
    templates: &[TemplateId],
    ctx: &mut Context<'_>,
    mut site: Site,
) -> Vec<Piece> {
    let rotation = random_rotation(&mut site.rng);
    let template = templates[site.rng.next_i32_bound(templates.len() as i32) as usize];
    let position = IVec3::new(
        ctx.chunk.min_block_x(),
        ShipwreckPiece::LAYOUT_FLOOR,
        ctx.chunk.min_block_z(),
    );
    let size = ctx.frozen.manifests[template.0 as usize]
        .size
        .map(i32::from);
    let bounds = bounding_box(
        ctx.frozen.manifests[template.0 as usize].size,
        position,
        rotation,
        Mirror::None,
        ShipwreckPiece::PIVOT,
    );
    let half_height = size[1] / 2;
    let height = if size[0] > REGION_FIT || size[1] > REGION_FIT {
        let x_span = bounds.max.x - bounds.min.x + 1;
        let z_span = bounds.max.z - bounds.min.z + 1;
        if is_beached {
            // The reference hands `getLowestY(context, minX, minZ, sizeX, sizeZ)`
            // its box as `(minX, xSpan, minZ, zSpan)`; the argument order is data.
            let lowest = lowest_y(ctx, bounds.min.x, x_span, bounds.min.z, z_span);
            lowest - half_height - site.rng.next_i32_bound(3)
        } else {
            corner_heights(ctx, bounds.min.x, x_span, bounds.min.z, z_span)
                .iter()
                .sum::<i32>()
                / 4
        }
    } else {
        let heightmap = heightmap(is_beached);
        let mut lowest = ctx.accessor_min_y + ctx.accessor_height;
        let mut sum = 0;
        for x in position.x..position.x + size[0] {
            for z in position.z..position.z + size[2] {
                let free = ctx.world.free_height(x, z, heightmap);
                sum += free;
                lowest = lowest.min(free);
            }
        }
        if is_beached {
            lowest - half_height - site.rng.next_i32_bound(3)
        } else {
            sum / (size[0] * size[2])
        }
    };
    vec![Piece::Shipwreck(ShipwreckPiece {
        template,
        position,
        rotation,
        is_beached,
        bounds,
        height,
    })]
}
