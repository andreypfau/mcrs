use bevy_math::IVec3;
use mcrs_minecraft_core::{BoundingBox, Mirror, ResourceLocation, Rotation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::template::{bounding_box, transform};

use super::on_top_of_chunk_centre;
use crate::OceanTemperature;
use crate::frozen::{OceanRuinConfig, TemplateId};
use crate::orient::random_rotation;
use crate::piece::{OceanRuinPiece, Piece};
use crate::site::{Context, Site, Stub};

pub const TEMPLATES: &[&str] = &[
    "underwater_ruin/warm_1",
    "underwater_ruin/warm_2",
    "underwater_ruin/warm_3",
    "underwater_ruin/warm_4",
    "underwater_ruin/warm_5",
    "underwater_ruin/warm_6",
    "underwater_ruin/warm_7",
    "underwater_ruin/warm_8",
    "underwater_ruin/brick_1",
    "underwater_ruin/brick_2",
    "underwater_ruin/brick_3",
    "underwater_ruin/brick_4",
    "underwater_ruin/brick_5",
    "underwater_ruin/brick_6",
    "underwater_ruin/brick_7",
    "underwater_ruin/brick_8",
    "underwater_ruin/cracked_1",
    "underwater_ruin/cracked_2",
    "underwater_ruin/cracked_3",
    "underwater_ruin/cracked_4",
    "underwater_ruin/cracked_5",
    "underwater_ruin/cracked_6",
    "underwater_ruin/cracked_7",
    "underwater_ruin/cracked_8",
    "underwater_ruin/mossy_1",
    "underwater_ruin/mossy_2",
    "underwater_ruin/mossy_3",
    "underwater_ruin/mossy_4",
    "underwater_ruin/mossy_5",
    "underwater_ruin/mossy_6",
    "underwater_ruin/mossy_7",
    "underwater_ruin/mossy_8",
    "underwater_ruin/big_brick_1",
    "underwater_ruin/big_brick_2",
    "underwater_ruin/big_brick_3",
    "underwater_ruin/big_brick_8",
    "underwater_ruin/big_mossy_1",
    "underwater_ruin/big_mossy_2",
    "underwater_ruin/big_mossy_3",
    "underwater_ruin/big_mossy_8",
    "underwater_ruin/big_cracked_1",
    "underwater_ruin/big_cracked_2",
    "underwater_ruin/big_cracked_3",
    "underwater_ruin/big_cracked_8",
    "underwater_ruin/big_warm_4",
    "underwater_ruin/big_warm_5",
    "underwater_ruin/big_warm_6",
    "underwater_ruin/big_warm_7",
];

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(
    _config: &OceanRuinConfig,
    ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    on_top_of_chunk_centre(ctx, HeightmapName::OceanFloorWg)
}

pub fn layout(config: &OceanRuinConfig, ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let rng = &mut site.rng;
    let position = IVec3::new(
        ctx.chunk.min_block_x(),
        OceanRuinPiece::LAYOUT_FLOOR,
        ctx.chunk.min_block_z(),
    );
    let rotation = random_rotation(rng);
    let mut pieces = Vec::new();
    let large = rng.next_f32() <= config.large_probability;
    let base_integrity = if large { 0.9 } else { 0.8 };
    add_piece(
        config,
        ctx,
        rng,
        &mut pieces,
        position,
        rotation,
        large,
        base_integrity,
    );
    if large && rng.next_f32() <= config.cluster_probability {
        add_cluster(config, ctx, rng, &mut pieces, position, rotation);
    }
    pieces
}

fn add_cluster(
    config: &OceanRuinConfig,
    ctx: &mut Context<'_>,
    rng: &mut LegacyRandom,
    pieces: &mut Vec<Piece>,
    parent: IVec3,
    rotation: Rotation,
) {
    let parent_corner =
        transform(IVec3::new(15, 0, 15), Mirror::None, rotation, IVec3::ZERO) + parent;
    let parent_box = BoundingBox::from_corners(parent.into(), parent_corner.into());
    let bottom_left = IVec3::new(
        parent.x.min(parent_corner.x),
        parent.y,
        parent.z.min(parent_corner.z),
    );
    let mut positions = all_positions(rng, bottom_left);
    let ruins = next_int_between(rng, 4, 8);
    for _ in 0..ruins {
        if positions.is_empty() {
            continue;
        }
        let index = rng.next_i32_bound(positions.len() as i32) as usize;
        let position = positions.remove(index);
        let next_rotation = random_rotation(rng);
        let corner = transform(
            IVec3::new(5, 0, 6),
            Mirror::None,
            next_rotation,
            IVec3::ZERO,
        ) + position;
        let next_box = BoundingBox::from_corners(position.into(), corner.into());
        if !next_box.intersects(parent_box) {
            add_piece(
                config,
                ctx,
                rng,
                pieces,
                position,
                next_rotation,
                false,
                0.8,
            );
        }
    }
}

/// `Mth.nextInt(random, min, max)`, inclusive of both ends.
fn next_int_between(rng: &mut LegacyRandom, min: i32, max: i32) -> i32 {
    if min >= max {
        min
    } else {
        rng.next_i32_bound(max - min + 1) + min
    }
}

fn all_positions(rng: &mut LegacyRandom, origin: IVec3) -> Vec<IVec3> {
    let mut n = |min, max| next_int_between(rng, min, max);
    let offsets = [
        [-16 + n(1, 8), 16 + n(1, 7)],
        [-16 + n(1, 8), n(1, 7)],
        [-16 + n(1, 8), -16 + n(4, 8)],
        [n(1, 7), 16 + n(1, 7)],
        [n(1, 7), -16 + n(4, 6)],
        [16 + n(1, 7), 16 + n(3, 8)],
        [16 + n(1, 7), n(1, 7)],
        [16 + n(1, 7), -16 + n(4, 8)],
    ];
    offsets
        .into_iter()
        .map(|[x, z]| origin + IVec3::new(x, 0, z))
        .collect()
}

/// The ruins of one family, in the order the reference's arrays list them.
fn templates(family: &str, large: bool) -> Vec<&'static str> {
    let prefix = if large {
        format!("underwater_ruin/big_{family}_")
    } else {
        format!("underwater_ruin/{family}_")
    };
    TEMPLATES
        .iter()
        .copied()
        .filter(|name| name.starts_with(&prefix))
        .collect()
}

fn template_id(ctx: &Context<'_>, name: &str) -> TemplateId {
    ctx.frozen.template_ids[&ResourceLocation::minecraft(name)]
}

#[allow(clippy::too_many_arguments)]
fn add_piece(
    config: &OceanRuinConfig,
    ctx: &mut Context<'_>,
    rng: &mut LegacyRandom,
    pieces: &mut Vec<Piece>,
    position: IVec3,
    rotation: Rotation,
    large: bool,
    base_integrity: f32,
) {
    match config.biome_temp {
        OceanTemperature::Warm => {
            let warm = templates("warm", large);
            let name = warm[rng.next_i32_bound(warm.len() as i32) as usize];
            pieces.push(piece(
                config,
                ctx,
                name,
                position,
                rotation,
                base_integrity,
                large,
            ));
        }
        OceanTemperature::Cold => {
            let bricks = templates("brick", large);
            let cracked = templates("cracked", large);
            let mossy = templates("mossy", large);
            let index = rng.next_i32_bound(bricks.len() as i32) as usize;
            pieces.push(piece(
                config,
                ctx,
                bricks[index],
                position,
                rotation,
                base_integrity,
                large,
            ));
            pieces.push(piece(
                config,
                ctx,
                cracked[index],
                position,
                rotation,
                0.7,
                large,
            ));
            pieces.push(piece(
                config,
                ctx,
                mossy[index],
                position,
                rotation,
                0.5,
                large,
            ));
        }
    }
}

fn piece(
    config: &OceanRuinConfig,
    ctx: &mut Context<'_>,
    name: &str,
    position: IVec3,
    rotation: Rotation,
    integrity: f32,
    large: bool,
) -> Piece {
    let template = template_id(ctx, name);
    let size = ctx.frozen.manifests[template.0 as usize].size;
    let bounds = bounding_box(size, position, rotation, Mirror::None, IVec3::ZERO);
    Piece::OceanRuin(OceanRuinPiece {
        template,
        position,
        rotation,
        bounds,
        integrity,
        biome_temp: config.biome_temp,
        large,
        floor_y: floor(ctx, position, rotation, size),
    })
}

/// `OceanRuinPiece.postProcess`'s height: the ocean floor at the template's
/// corner, lowered to one above the footprint's deepest column when the
/// footprint is mostly a drop of more than two, read here from the base
/// columns rather than the live world.
fn floor(ctx: &mut Context<'_>, position: IVec3, rotation: Rotation, size: [u16; 3]) -> i32 {
    let height = ctx
        .world
        .free_height(position.x, position.z, HeightmapName::OceanFloorWg);
    let pos = IVec3::new(position.x, height, position.z);
    let corner = transform(
        IVec3::new(i32::from(size[0]) - 1, 0, i32::from(size[2]) - 1),
        Mirror::None,
        rotation,
        IVec3::ZERO,
    ) + pos;
    let floor_limit = ctx.height.min_y + 1;
    let top_y = pos.y - 1;
    let (air, water) = {
        let states = ctx.world.states();
        (states.air_states.clone(), states.water_fluid.clone())
    };
    let mut min_y = 512;
    let mut area = 0;
    for x in pos.x.min(corner.x)..=pos.x.max(corner.x) {
        for z in pos.z.min(corner.z)..=pos.z.max(corner.z) {
            let column = ctx.world.base_column(x, z);
            let mut floor_y = pos.y - 1;
            let mut state = column.block(floor_y).0 as usize;
            while (air.contains(state) || water.contains(state)) && floor_y > floor_limit {
                floor_y -= 1;
                state = column.block(floor_y).0 as usize;
            }
            min_y = min_y.min(floor_y);
            if floor_y < top_y - 2 {
                area += 1;
            }
        }
    }
    let width = (pos.x - corner.x).abs();
    if top_y - min_y > 2 && area > width - 2 {
        min_y + 1
    } else {
        pos.y
    }
}
