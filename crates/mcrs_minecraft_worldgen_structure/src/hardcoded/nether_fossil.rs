use bevy_math::IVec3;
use mcrs_minecraft_core::Mirror;
use mcrs_minecraft_core::value_provider::HeightProvider;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::template::bounding_box;

use crate::frozen::TemplateId;
use crate::orient::random_rotation;
use crate::piece::{NetherFossilPiece, Piece};
use crate::site::{Context, Site, Stub};

pub const TEMPLATES: &[&str] = &[
    "nether_fossils/fossil_1",
    "nether_fossils/fossil_2",
    "nether_fossils/fossil_3",
    "nether_fossils/fossil_4",
    "nether_fossils/fossil_5",
    "nether_fossils/fossil_6",
    "nether_fossils/fossil_7",
    "nether_fossils/fossil_8",
    "nether_fossils/fossil_9",
    "nether_fossils/fossil_10",
    "nether_fossils/fossil_11",
    "nether_fossils/fossil_12",
    "nether_fossils/fossil_13",
    "nether_fossils/fossil_14",
];

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(
    height: &HeightProvider,
    ctx: &mut Context<'_>,
    rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    let x = ctx.chunk.min_block_x() + rng.next_i32_bound(16);
    let z = ctx.chunk.min_block_z() + rng.next_i32_bound(16);
    let sea_level = ctx.height.sea_level;
    let mut y = height.sample(rng, ctx.height);
    let (air, sturdy_up) = {
        let states = ctx.world.states();
        (states.air_states.clone(), states.sturdy_up.clone())
    };
    let column = ctx.world.base_column(x, z);
    // ponytail: the reference also accepts soul sand under the fossil, whose
    // collision shape is not a full block; a base column holds only the noise
    // settings' default block, so that matters for a datapack whose default
    // block is soul sand.
    while y > sea_level {
        let current = column.block(y);
        y -= 1;
        let below = column.block(y);
        if air.contains(current.0 as usize) && sturdy_up.contains(below.0 as usize) {
            break;
        }
    }
    (y > sea_level).then_some((IVec3::new(x, y, z), Stub::Fossil))
}

pub fn layout(templates: &[TemplateId], ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let rotation = random_rotation(&mut site.rng);
    let template = templates[site.rng.next_i32_bound(templates.len() as i32) as usize];
    let size = ctx.frozen.manifests[template.0 as usize].size;
    vec![Piece::NetherFossil(NetherFossilPiece {
        template,
        position: site.position,
        rotation,
        bounds: bounding_box(size, site.position, rotation, Mirror::None, IVec3::ZERO),
    })]
}
