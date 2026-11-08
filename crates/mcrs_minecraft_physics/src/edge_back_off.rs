use glam::DVec3;

use crate::Aabb;
use crate::BodyView;
use crate::CollisionWorld;
use crate::QueryEntity;
use crate::Scratch;
use crate::View;
use crate::query;
use crate::tolerance::COLLISION;

const STEP: f64 = 0.05;

/// The state of a sneaking player that the edge back-off reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Edge<'a> {
    pub(crate) entity: &'a QueryEntity,
    pub(crate) bounds: Aabb,
    pub(crate) on_ground: bool,
    pub(crate) fall_distance: f64,
    pub(crate) step_height: f32,
}

/// Shortens the horizontal displacement so that a sneaking player does not fall from the edge
/// of a block. It never makes a component larger and never changes its sign.
pub(crate) fn back_off<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    edge: &Edge<'_>,
    d: DVec3,
    scratch: &mut Scratch,
) -> DVec3 {
    let height = f64::from(edge.step_height);
    let above_ground = edge.on_ground
        || (edge.fall_distance < height
            && !can_fall(view, edge, height - edge.fall_distance, 0.0, 0.0, scratch));
    if !above_ground {
        return d;
    }
    let sign_x = d.x.signum();
    let sign_z = d.z.signum();
    let mut x = d.x;
    let mut z = d.z;
    while x != 0.0 && can_fall(view, edge, height, x, 0.0, scratch) {
        x = shorten(x, sign_x);
    }
    while z != 0.0 && can_fall(view, edge, height, 0.0, z, scratch) {
        z = shorten(z, sign_z);
    }
    while x != 0.0 && z != 0.0 && can_fall(view, edge, height, x, z, scratch) {
        x = shorten(x, sign_x);
        z = shorten(z, sign_z);
    }
    DVec3::new(x, d.y, z)
}

fn shorten(value: f64, sign: f64) -> f64 {
    if value.abs() <= STEP {
        0.0
    } else {
        value - sign * STEP
    }
}

/// True when the box, moved by the offset, can fall `depth` with no collision.
fn can_fall<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    edge: &Edge<'_>,
    depth: f64,
    dx: f64,
    dz: f64,
    scratch: &mut Scratch,
) -> bool {
    let bounds = edge.bounds;
    let test = Aabb::new(
        DVec3::new(
            bounds.min.x + COLLISION + dx,
            bounds.min.y - depth - COLLISION,
            bounds.min.z + COLLISION + dz,
        ),
        DVec3::new(
            bounds.max.x - COLLISION + dx,
            bounds.min.y,
            bounds.max.z - COLLISION + dz,
        ),
    );
    !query::intersects(view, edge.entity, test, &mut scratch.entities)
}
