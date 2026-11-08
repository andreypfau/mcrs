use core::ops::ControlFlow;

use glam::DVec3;

use crate::Aabb;
use crate::BodyView;
use crate::Collider;
use crate::ColliderSource;
use crate::CollisionWorld;
use crate::EntityCollider;
use crate::QueryEntity;
use crate::QueryVariant;
use crate::SourcedCollider;
use crate::View;
use crate::block_scan;
use crate::block_scan::ShapeContext;
use crate::tolerance::COLLISION;

/// Appends each collider of the query box `query` for the entity, in the order of the collision
/// algorithm: the entity colliders, the world border, then the blocks in the scan order.
///
/// `feet` replaces the bottom of the entity for the context shapes, for example the `y` of the
/// old position, which gives a scaffolding shape from before the move.
pub fn query_colliders<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    entity: &QueryEntity,
    query: Aabb,
    feet: Option<f64>,
    variant: QueryVariant,
    out: &mut Vec<SourcedCollider>,
) {
    let mut entities = Vec::new();
    entity_colliders(view.bodies, entity, &query, &mut entities);
    out.extend(entities.iter().map(|found| SourcedCollider {
        collider: Collider::Entity(found.bounds),
        source: ColliderSource::Entity(found.id),
    }));
    if variant == QueryVariant::Collision
        && let Some(border) = border_collider(view.world, entity, &query)
    {
        out.push(SourcedCollider {
            collider: border,
            source: ColliderSource::Border,
        });
    }
    let context = ShapeContext {
        entity,
        feet: feet.unwrap_or(entity.position.y),
        variant,
    };
    let _ = block_scan::scan(
        view.world,
        &view.tables.collision,
        &context,
        &query,
        |collider, state| {
            if let Collider::Block { at, .. } = collider {
                out.push(SourcedCollider {
                    collider,
                    source: ColliderSource::Block { pos: at, state },
                });
            }
            ControlFlow::Continue(())
        },
    );
}

/// True when a block shape, an entity collider or the world border overlaps the box with a
/// volume that is more than zero. The caller shrinks the box when its rule asks for it.
#[must_use]
pub fn intersects<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    entity: &QueryEntity,
    bounds: Aabb,
    entities: &mut Vec<EntityCollider>,
) -> bool {
    entities.clear();
    entity_colliders(view.bodies, entity, &bounds, entities);
    let table = &view.tables.collision;
    if entities
        .iter()
        .any(|found| Collider::Entity(found.bounds).intersects(table, &bounds))
    {
        return true;
    }
    if border_collider(view.world, entity, &bounds)
        .is_some_and(|border| border.intersects(table, &bounds))
    {
        return true;
    }
    let context = ShapeContext {
        entity,
        feet: entity.position.y,
        variant: QueryVariant::Collision,
    };
    block_scan::scan(view.world, table, &context, &bounds, |_, _| {
        ControlFlow::Break(())
    })
    .is_break()
}

/// Appends the entities that can stop the box in `query`.
pub(crate) fn entity_colliders<B: BodyView>(
    bodies: &B,
    entity: &QueryEntity,
    query: &Aabb,
    out: &mut Vec<EntityCollider>,
) {
    if query.size().element_sum() / 3.0 < COLLISION {
        return;
    }
    bodies.colliders(query.inflate(DVec3::splat(COLLISION)), entity.key, out);
}

/// The world border as a collider of the query, when the entity is near it.
pub(crate) fn border_collider<W: CollisionWorld>(
    world: &W,
    entity: &QueryEntity,
    query: &Aabb,
) -> Option<Collider> {
    let border = world.border()?;
    let size = query.size();
    let margin = size.x.abs().max(size.z.abs()).max(1.0);
    let near = border.distance_inside(entity.position) < 2.0 * margin
        && border.contains_with_margin(entity.position, margin);
    near.then(|| Collider::Border(border.region()))
}
