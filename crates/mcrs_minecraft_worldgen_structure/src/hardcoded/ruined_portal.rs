use bevy_math::IVec3;
use mcrs_minecraft_core::{BoundingBox, Mirror, Rotation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::template::bounding_box;

use crate::frozen::TemplateId;
use crate::orient::random_rotation;
use crate::piece::{Piece, PortalProperties, RuinedPortalPiece};
use crate::site::{Context, Site, Stub};
use crate::{PortalPlacement, RuinedPortalSetup};

pub const TEMPLATES: &[&str] = &[
    "ruined_portal/portal_1",
    "ruined_portal/portal_2",
    "ruined_portal/portal_3",
    "ruined_portal/portal_4",
    "ruined_portal/portal_5",
    "ruined_portal/portal_6",
    "ruined_portal/portal_7",
    "ruined_portal/portal_8",
    "ruined_portal/portal_9",
    "ruined_portal/portal_10",
    "ruined_portal/giant_portal_1",
    "ruined_portal/giant_portal_2",
    "ruined_portal/giant_portal_3",
];

pub const PORTALS: &[&str] = TEMPLATES.split_at(10).0;
pub const GIANT_PORTALS: &[&str] = TEMPLATES.split_at(10).1;

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

const GIANT_PORTAL_PROBABILITY: f32 = 0.05;
const MIN_Y_INDEX: i32 = 15;

pub fn heightmap(placement: PortalPlacement) -> HeightmapName {
    match placement {
        PortalPlacement::OnOceanFloor => HeightmapName::OceanFloorWg,
        _ => HeightmapName::WorldSurfaceWg,
    }
}

pub fn site(
    setups: &[RuinedPortalSetup],
    portals: &[TemplateId],
    giant_portals: &[TemplateId],
    ctx: &mut Context<'_>,
    rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    let setup = if setups.len() > 1 {
        let total: f32 = setups.iter().map(|s| s.weight.0 as f32).sum();
        let mut pick = rng.next_f32();
        setups
            .iter()
            .position(|s| {
                pick -= s.weight.0 as f32 / total;
                pick < 0.0
            })
            .expect("a draw below one lands on a setup")
    } else {
        0
    };
    let chosen = &setups[setup];
    let air_pocket = sample(rng, chosen.air_pocket_probability.0 as f32);
    let template = if rng.next_f32() < GIANT_PORTAL_PROBABILITY {
        giant_portals[rng.next_i32_bound(giant_portals.len() as i32) as usize]
    } else {
        portals[rng.next_i32_bound(portals.len() as i32) as usize]
    };
    let rotation = random_rotation(rng);
    let mirror = if rng.next_f32() < 0.5 {
        Mirror::None
    } else {
        Mirror::FrontBack
    };
    let base = IVec3::new(ctx.chunk.min_block_x(), 0, ctx.chunk.min_block_z());
    let bounds = template_bounds(ctx, template, base, rotation, mirror);
    let centre = *bounds.min + (*bounds.max - *bounds.min + IVec3::ONE) / 2;
    let surface_y = ctx
        .world
        .free_height(centre.x, centre.z, heightmap(chosen.placement))
        - 1;
    let min_y = ctx.accessor_min_y + MIN_Y_INDEX;
    let y_span = bounds.max.y - bounds.min.y + 1;
    let start_y = match chosen.placement {
        PortalPlacement::InNether => {
            if air_pocket {
                rng.next_int_between_inclusive(32, 100)
            } else if rng.next_f32() < 0.5 {
                rng.next_int_between_inclusive(27, 29)
            } else {
                rng.next_int_between_inclusive(29, 100)
            }
        }
        PortalPlacement::InMountain => within_interval(rng, 70, surface_y - y_span),
        PortalPlacement::Underground => within_interval(rng, min_y, surface_y - y_span),
        PortalPlacement::PartlyBuried => surface_y - y_span + rng.next_int_between_inclusive(2, 8),
        PortalPlacement::OnLandSurface | PortalPlacement::OnOceanFloor => surface_y,
    };
    let corners = [
        (bounds.min.x, bounds.min.z),
        (bounds.max.x, bounds.min.z),
        (bounds.min.x, bounds.max.z),
        (bounds.max.x, bounds.max.z),
    ];
    let floor = if chosen.placement == PortalPlacement::OnOceanFloor {
        HeightmapName::OceanFloorWg
    } else {
        HeightmapName::WorldSurfaceWg
    };
    let mut y = start_y;
    while y > min_y {
        let mut on_solid_ground = 0;
        for (x, z) in corners {
            let state = ctx.world.base_column(x, z).block(y);
            if ctx.world.opaque(state, floor) {
                on_solid_ground += 1;
                if on_solid_ground == 3 {
                    return Some(portal_site(
                        base, y, setup, air_pocket, template, rotation, mirror,
                    ));
                }
            }
        }
        y -= 1;
    }
    Some(portal_site(
        base, y, setup, air_pocket, template, rotation, mirror,
    ))
}

fn portal_site(
    base: IVec3,
    y: i32,
    setup: usize,
    air_pocket: bool,
    template: TemplateId,
    rotation: Rotation,
    mirror: Mirror,
) -> (IVec3, Stub) {
    (
        IVec3::new(base.x, y, base.z),
        Stub::Portal {
            setup,
            air_pocket,
            template,
            rotation,
            mirror,
        },
    )
}

fn template_bounds(
    ctx: &Context<'_>,
    template: TemplateId,
    position: IVec3,
    rotation: Rotation,
    mirror: Mirror,
) -> BoundingBox {
    let size = ctx.frozen.manifests[template.0 as usize].size;
    let pivot = IVec3::new(i32::from(size[0]) / 2, 0, i32::from(size[2]) / 2);
    bounding_box(size, position, rotation, mirror, pivot)
}

fn sample(rng: &mut LegacyRandom, limit: f32) -> bool {
    if limit == 0.0 {
        false
    } else if limit == 1.0 {
        true
    } else {
        rng.next_f32() < limit
    }
}

fn within_interval(rng: &mut LegacyRandom, min_preferred: i32, max: i32) -> i32 {
    if min_preferred < max {
        rng.next_int_between_inclusive(min_preferred, max)
    } else {
        max
    }
}

pub fn layout(setups: &[RuinedPortalSetup], ctx: &mut Context<'_>, site: Site) -> Vec<Piece> {
    let Stub::Portal {
        setup,
        air_pocket,
        template,
        rotation,
        mirror,
    } = site.stub
    else {
        unreachable!("a ruined portal site carries a portal stub")
    };
    let setup = &setups[setup];
    let cold = setup.can_be_cold
        && ctx
            .world
            .cold_enough_to_snow(site.position, ctx.height.sea_level);
    vec![Piece::RuinedPortal(RuinedPortalPiece {
        template,
        position: site.position,
        rotation,
        mirror,
        bounds: template_bounds(ctx, template, site.position, rotation, mirror),
        placement: setup.placement,
        properties: PortalProperties {
            cold,
            mossiness: setup.mossiness.0 as f32,
            air_pocket,
            overgrown: setup.overgrown,
            vines: setup.vines,
            replace_with_blackstone: setup.replace_with_blackstone,
        },
    })]
}
