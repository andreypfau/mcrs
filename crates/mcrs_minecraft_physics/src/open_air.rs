use glam::DVec3;
use mcrs_voxel::SectionPos;

use crate::Aabb;
use crate::BodyView;
use crate::CollisionSection;
use crate::CollisionWorld;
use crate::EntityCollider;
use crate::EntityState;
use crate::MotionRow;
use crate::MovePath;
use crate::QueryEntity;
use crate::Rest;
use crate::SectionLookup;
use crate::View;
use crate::block_scan::floor_block;
use crate::query;
use crate::tick_entity::read_zone;

/// True when the next tick of the entity meets no block, no collider and no border: the read
/// zone is in loaded sections of air only. Then the open-air step gives the result of the full
/// tick.
#[must_use]
pub fn in_open_air<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    row: &MotionRow,
    entity: &EntityState,
    scratch_entities: &mut Vec<EntityCollider>,
) -> bool {
    let body = &entity.body;
    if row.category.is_living()
        || body.flags.on_ground
        || body.stuck != DVec3::ZERO
        || !matches!(entity.rest, Rest::Awake | Rest::Candidate(_))
    {
        return false;
    }
    let gravity = if entity.no_gravity { 0.0 } else { row.gravity };
    let d = DVec3::new(body.velocity.x, body.velocity.y - gravity, body.velocity.z);
    let bounds = Aabb::of_body(body.position, row.dimensions.width, row.dimensions.height);
    let zone = read_zone(bounds, d, 0.0);
    if !air_only(view.world, &zone) {
        return false;
    }
    if let Some(border) = view.world.border() {
        let margin = (zone.size().x.max(zone.size().z) + 2.0) * 2.0;
        if border.distance_inside(body.position) <= margin {
            return false;
        }
    }
    scratch_entities.clear();
    let entity_query = QueryEntity {
        key: entity.key,
        position: body.position,
        fall_distance: body.fall_distance,
        shift: false,
        powder_snow_solid: row.powder_snow_solid,
        walks_on_powder_snow: false,
    };
    query::entity_colliders(view.bodies, &entity_query, &zone, scratch_entities);
    scratch_entities.is_empty()
}

/// The physics part of one tick in open air.
pub(crate) fn air_step(row: &MotionRow, entity: &mut EntityState) {
    let drag = f64::from(0.98_f32);
    let gravity = if entity.no_gravity { 0.0 } else { row.gravity };
    let body = &mut entity.body;
    body.velocity.y -= gravity;
    let start = body.position;
    body.position += body.velocity;
    if body.position != start && body.velocity.y < 0.0 {
        body.fall_distance -= body.velocity.y;
    }
    body.velocity *= drag;
    body.flags.horizontal_collision = false;
    body.flags.minor_collision = false;
    body.flags.vertical_collision = false;
    body.flags.below_collision = false;
    body.flags.on_ground = false;
    body.support = None;
    body.support_missed = false;
    entity.last_path = MovePath {
        start,
        end: body.position,
    };
}

/// True when the box is in loaded sections whose blocks are all air.
fn air_only<W: CollisionWorld>(world: &W, zone: &Aabb) -> bool {
    let low = zone
        .min
        .to_array()
        .map(|value| floor_block(value - 1.0e-7) - 1);
    let high = zone
        .max
        .to_array()
        .map(|value| floor_block(value + 1.0e-7) + 1);
    let [x0, y0, z0] = low.map(|value| value >> 4);
    let [x1, y1, z1] = high.map(|value| value >> 4);
    for sy in y0..=y1 {
        for sz in z0..=z1 {
            for sx in x0..=x1 {
                let clear = match world.section(SectionPos::new(sx, sy, sz)) {
                    SectionLookup::Loaded(section) => {
                        section.masks().is_some_and(|masks| masks.air_only)
                    }
                    SectionLookup::Outside => true,
                    SectionLookup::Unloaded => false,
                };
                if !clear {
                    return false;
                }
            }
        }
    }
    true
}
