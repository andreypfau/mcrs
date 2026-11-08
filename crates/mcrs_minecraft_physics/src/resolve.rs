use core::ops::ControlFlow;

use glam::DVec3;

use crate::Aabb;
use crate::BodyView;
use crate::Collider;
use crate::CollisionTable;
use crate::CollisionWorld;
use crate::QueryEntity;
use crate::QueryVariant;
use crate::Scratch;
use crate::View;
use crate::block_scan;
use crate::block_scan::ShapeContext;
use crate::collide::collide;
use crate::query;
use crate::tolerance::CLIP;

/// One displacement to resolve for a box, with the values of the step-up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    pub entity: QueryEntity,
    pub bounds: Aabb,
    pub displacement: DVec3,
    pub step_height: f32,
    /// The on-ground flag from before the move.
    pub on_ground: bool,
}

/// The displacement that the colliders allow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    pub displacement: DVec3,
    /// Set when a step-up gave the displacement.
    pub step: Option<StepUp>,
}

/// The values of a step-up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepUp {
    /// The displacement without the step-up.
    pub plain: DVec3,
    /// `base.ymin - box.ymin`: the lift of the base box before the step.
    pub base_lift: f64,
}

/// Resolves a displacement against the blocks, the entity colliders and the world border, with
/// step-up. It changes no state, so a checker can call it for a candidate move.
#[must_use]
pub fn resolve<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    probe: &Probe,
    scratch: &mut Scratch,
) -> Resolved {
    let d = probe.displacement;
    if d.length_squared() == 0.0 {
        return Resolved {
            displacement: d,
            step: None,
        };
    }
    let table = &view.tables.collision;
    let entity = &probe.entity;
    let query = probe.bounds.expand_towards(d);
    let height = f64::from(probe.step_height);
    let mut entity_query = query;
    entity_query.max.y += height;
    scratch.entities.clear();
    query::entity_colliders(view.bodies, entity, &entity_query, &mut scratch.entities);
    scratch.colliders.clear();
    scratch.colliders.extend(
        scratch
            .entities
            .iter()
            .map(|found| Collider::Entity(found.bounds)),
    );
    let entity_count = scratch.colliders.len();
    gather_static(view.world, table, entity, &query, &mut scratch.colliders);
    let plain = collide(table, &scratch.colliders, probe.bounds, d);
    let lands = plain.y != d.y && d.y < 0.0;
    let blocked = plain.x != d.x || plain.z != d.z;
    if probe.step_height <= 0.0 || !(lands || probe.on_ground) || !blocked {
        return Resolved {
            displacement: plain,
            step: None,
        };
    }
    let base = if lands {
        probe.bounds.translate(DVec3::new(0.0, plain.y, 0.0))
    } else {
        probe.bounds
    };
    let mut area = base.expand_towards(DVec3::new(d.x, height, d.z));
    if !lands {
        area.min.y -= CLIP;
    }
    scratch.step_colliders.clear();
    scratch
        .step_colliders
        .extend(scratch.colliders.iter().take(entity_count).copied());
    gather_static(
        view.world,
        table,
        entity,
        &area,
        &mut scratch.step_colliders,
    );
    candidates(table, scratch, base.min.y, plain.y, probe.step_height);
    let plain_length = plain.x * plain.x + plain.z * plain.z;
    for &candidate in &scratch.candidates {
        let stepped = collide(
            table,
            &scratch.step_colliders,
            base,
            DVec3::new(d.x, f64::from(candidate), d.z),
        );
        if stepped.x * stepped.x + stepped.z * stepped.z > plain_length {
            let base_lift = base.min.y - probe.bounds.min.y;
            return Resolved {
                displacement: stepped - DVec3::new(0.0, probe.bounds.min.y - base.min.y, 0.0),
                step: Some(StepUp { plain, base_lift }),
            };
        }
    }
    Resolved {
        displacement: plain,
        step: None,
    }
}

/// Appends the world border and the block colliders of the query.
fn gather_static<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    entity: &QueryEntity,
    query: &Aabb,
    out: &mut Vec<Collider>,
) {
    if let Some(border) = query::border_collider(world, entity, query) {
        out.push(border);
    }
    let context = ShapeContext {
        entity,
        feet: entity.position.y,
        variant: QueryVariant::Collision,
    };
    let _ = block_scan::scan(world, table, &context, query, |collider, _| {
        out.push(collider);
        ControlFlow::Continue(())
    });
}

/// The step heights to try, from the `y` grid lines of the step colliders, lowest first.
fn candidates(
    table: &CollisionTable,
    scratch: &mut Scratch,
    base_y: f64,
    resolved_y: f64,
    height: f32,
) {
    let Scratch {
        step_colliders,
        lines,
        candidates,
        ..
    } = scratch;
    candidates.clear();
    let resolved = resolved_y as f32;
    for collider in step_colliders.iter() {
        collider.write_y_lines(table, lines);
        for &line in lines.iter() {
            let candidate = (line - base_y) as f32;
            if candidate < 0.0 || candidate == resolved {
                continue;
            }
            if candidate > height {
                break;
            }
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort_by(f32::total_cmp);
}
