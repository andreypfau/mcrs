use glam::DVec3;
use mcrs_voxel::SectionPos;

use crate::Aabb;
use crate::BodyView;
use crate::Category;
use crate::CollisionSection;
use crate::CollisionWorld;
use crate::EntityCollider;
use crate::EntityState;
use crate::MotionRow;
use crate::QueryEntity;
use crate::Rest;
use crate::SectionLookup;
use crate::TickOutcome;
use crate::View;
use crate::block_scan::floor_block;
use crate::query;
use crate::rest::same_bits;
use crate::tick_entity::read_zone;

/// The rows of one batch.
pub const LANES: usize = 4;

/// True when the next tick of the entity meets no block, no collider and no border: the read
/// zone is in loaded sections of air only. Then [`tick_in_air`] gives the result of
/// [`tick_entity`](crate::tick_entity) for it.
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

/// One tick of four entities of one category in open air. The result is the result of
/// [`tick_entity`](crate::tick_entity), bit for bit. A caller that wakes a row sets its rest to
/// [`Rest::Awake`] first, as the scalar tick does.
pub fn tick_in_air(row: &MotionRow, lanes: &mut [EntityState; LANES]) -> [TickOutcome; LANES] {
    let mut outcomes = [TickOutcome::default(); LANES];
    for (entity, outcome) in lanes.iter_mut().zip(&mut outcomes) {
        let start = entity.body;
        air_step(row, entity);
        let item = row.category == Category::Item;
        if item
            && !entity
                .age
                .wrapping_add(u32::from(entity.phase))
                .is_multiple_of(4)
        {
            continue;
        }
        let reference = match entity.rest {
            Rest::Candidate(state) if item => Some(state),
            _ if item => None,
            _ => Some(start),
        };
        outcome.fixed_point = reference.is_some_and(|state| same_bits(&state, &entity.body));
        entity.rest = if outcome.fixed_point {
            Rest::Resting(entity.body)
        } else if item {
            Rest::Candidate(entity.body)
        } else {
            Rest::Awake
        };
    }
    outcomes
}

/// The physics part of one tick in open air, for one entity: the operations of one lane of
/// [`tick_in_air`].
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
