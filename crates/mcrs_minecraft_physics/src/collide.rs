use glam::DVec3;

use crate::Aabb;
use crate::Axis;
use crate::Collider;
use crate::CollisionTable;
use crate::tolerance::COLLISION;

/// Resolves the displacement of the box against the colliders, one axis at a time: `y` first,
/// then the axis with the larger horizontal component last.
#[must_use]
pub fn collide(table: &CollisionTable, colliders: &[Collider], bounds: Aabb, d: DVec3) -> DVec3 {
    if colliders.is_empty() {
        return d;
    }
    let order = if d.x.abs() < d.z.abs() {
        [Axis::Y, Axis::Z, Axis::X]
    } else {
        [Axis::Y, Axis::X, Axis::Z]
    };
    let mut moved = bounds;
    let mut resolved = DVec3::ZERO;
    for axis in order {
        let requested = axis.of(d);
        if requested == 0.0 {
            continue;
        }
        let clipped = clip_all(table, colliders, axis, &moved, requested);
        resolved = axis.with(resolved, clipped);
        moved = moved.translate(axis.with(DVec3::ZERO, clipped));
    }
    resolved
}

/// Clips the distance against each collider in turn. A distance below the tolerance before a
/// collider ends the clip at exactly zero; a result just past zero after the last collider stays.
fn clip_all(
    table: &CollisionTable,
    colliders: &[Collider],
    axis: Axis,
    mover: &Aabb,
    t: f64,
) -> f64 {
    let mut distance = t;
    for collider in colliders {
        if distance.abs() < COLLISION {
            return 0.0;
        }
        distance = collider.clip(table, axis, mover, distance);
    }
    distance
}
