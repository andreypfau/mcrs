use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::Aabb;
use crate::BlockFlags;
use crate::Body;
use crate::BodyView;
use crate::ClippedAxes;
use crate::CollisionTable;
use crate::CollisionWorld;
use crate::MoveFlags;
use crate::MoveOutcome;
use crate::Mover;
use crate::MoverKind;
use crate::Physics;
use crate::Probe;
use crate::QueryVariant;
use crate::Scratch;
use crate::View;
use crate::block_scan::ShapeContext;
use crate::block_scan::block_at;
use crate::block_scan::state_at;
use crate::edge_back_off;
use crate::edge_back_off::Edge;
use crate::resolve;
use crate::support;
use crate::support::EFFECT_OFFSET;
use crate::support::MOVEMENT_OFFSET;
use crate::tolerance::CLIP;
use crate::tolerance::COLLISION;

/// The largest angle in radians between the intended direction and the move for a minor
/// collision: `0.13962634F`, 8 degrees.
const MINOR_ANGLE: f32 = 0.139_626_34;

/// Moves the body by `displacement`: the stuck multiplier, the edge back-off, the collision with
/// step-up, the flags and the supporting block, the velocity after a collision and the speed
/// factor.
pub fn move_body<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    mover: &Mover,
    body: &mut Body,
    kind: MoverKind,
    displacement: DVec3,
    scratch: &mut Scratch,
) -> MoveOutcome {
    let mut d = displacement;
    if mover.physics == Physics::Pass {
        body.position += d;
        body.flags.horizontal_collision = false;
        body.flags.vertical_collision = false;
        body.flags.below_collision = false;
        body.flags.minor_collision = false;
        return MoveOutcome {
            requested: d,
            resolved: d,
            step: None,
            clipped: ClippedAxes::default(),
            bounce: None,
        };
    }
    if body.stuck.length_squared() > COLLISION {
        if kind != MoverKind::Piston {
            d *= body.stuck;
        }
        body.stuck = DVec3::ZERO;
        body.velocity = DVec3::ZERO;
    }
    let bounds = Aabb::of_body(body.position, mover.width, mover.height);
    let entity = mover.query_entity(body);
    if mover.keeps_to_edges
        && mover.shift
        && d.y <= 0.0
        && matches!(kind, MoverKind::Own | MoverKind::Player)
    {
        let edge = Edge {
            entity: &entity,
            bounds,
            on_ground: body.flags.on_ground,
            fall_distance: body.fall_distance,
            step_height: mover.step_height,
        };
        d = edge_back_off::back_off(view, &edge, d, scratch);
    }
    let probe = Probe {
        entity,
        bounds,
        displacement: d,
        step_height: mover.step_height,
        on_ground: body.flags.on_ground,
    };
    let resolved = resolve::resolve(view, &probe, scratch);
    let r = resolved.displacement;
    let r_squared = r.length_squared();
    if r_squared > COLLISION || d.length_squared() - r_squared < COLLISION {
        body.position += r;
    }
    let clipped = ClippedAxes {
        x: (d.x - r.x).abs() >= CLIP,
        z: (d.z - r.z).abs() >= CLIP,
    };
    let vertical_changed = set_flags(&mut body.flags, mover, d, r, clipped);
    let table = &view.tables.collision;
    if vertical_changed {
        find_support(view, mover, body, r);
    }
    let bounce = if mover.side.has_authority() {
        collide_velocity(view.world, table, mover, body, d, r, clipped)
    } else {
        None
    };
    apply_speed_factor(view.world, table, mover, body);
    MoveOutcome {
        requested: d,
        resolved: r,
        step: resolved.step,
        clipped,
        bounce,
    }
}

/// Finds the supporting block after a move that changed the vertical flags.
fn find_support<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    mover: &Mover,
    body: &mut Body,
    r: DVec3,
) {
    if !body.flags.on_ground {
        body.support = None;
        body.support_missed = false;
        return;
    }
    let bounds = Aabb::of_body(body.position, mover.width, mover.height);
    let entity = mover.query_entity(body);
    let context = ShapeContext {
        entity: &entity,
        feet: body.position.y,
        variant: QueryVariant::Collision,
    };
    let found = support::find_support(
        view.world,
        &view.tables.collision,
        &context,
        &bounds,
        r,
        body.support_missed,
    );
    body.support = found.block;
    body.support_missed = found.missed;
}

/// Sets the collision flags and returns true when the vertical flags changed.
fn set_flags(
    flags: &mut MoveFlags,
    mover: &Mover,
    d: DVec3,
    r: DVec3,
    clipped: ClippedAxes,
) -> bool {
    flags.horizontal_collision = clipped.x || clipped.z;
    let vertical_changed = d.y != 0.0 || mover.side.has_authority();
    if vertical_changed {
        flags.vertical_collision = r.y != d.y;
        flags.below_collision = flags.vertical_collision && d.y < 0.0;
        flags.on_ground = flags.below_collision;
    }
    flags.minor_collision = flags.horizontal_collision
        && mover
            .intent
            .is_some_and(|intent| is_minor(intent, DVec3::new(r.x, 0.0, r.z)));
    vertical_changed
}

/// True when the angle between the intended direction and the horizontal move is less than 8
/// degrees. A quotient above 1 from rounding gives NaN and false, as the client computes it.
fn is_minor(intent: DVec3, moved: DVec3) -> bool {
    let intent_squared = intent.length_squared();
    let moved_squared = moved.length_squared();
    if intent_squared < CLIP || moved_squared < CLIP {
        return false;
    }
    let cosine = intent.dot(moved) / (intent_squared * moved_squared).sqrt();
    cosine.acos() < f64::from(MINOR_ANGLE)
}

/// The velocity after a collision: the clipped components lose their speed, or bounce with the
/// restitution.
fn collide_velocity<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    mover: &Mover,
    body: &mut Body,
    d: DVec3,
    r: DVec3,
    clipped: ClippedAxes,
) -> Option<BlockPos> {
    let flags = body.flags;
    if !((d.y != 0.0 && flags.vertical_collision) || flags.horizontal_collision) {
        return None;
    }
    let restitution = if mover.shift { 0.0 } else { mover.restitution };
    let before = body.velocity;
    let effect = offset_block(world, table, body, EFFECT_OFFSET);
    let mut bounced = false;
    if clipped.x {
        body.velocity.x = -before.x * restitution;
        bounced |= restitution > 0.0;
    }
    if clipped.z {
        body.velocity.z = -before.z * restitution;
        bounced |= restitution > 0.0;
    }
    if flags.vertical_collision {
        let mut vertical = restitution;
        if flags.below_collision {
            let block = table.state(state_at(world, effect));
            vertical = if -before.y <= mover.gravity
                || mover.shift
                || block.flags.contains(BlockFlags::SUPPRESSES_BOUNCE)
            {
                0.0
            } else {
                let block_bounce = block.motion.bounce * mover.block_bounce_factor;
                vertical.max(f64::from(block_bounce))
            };
        }
        body.velocity.y = if vertical > 0.0 {
            let part = r.y / before.y;
            let drag = f64::from(mover.vertical_drag);
            (part * mover.gravity - before.y) * (1.0 + part * (drag - 1.0)) * vertical
        } else {
            (0.0 - before.y) * 1.0 * 0.0
        };
        bounced |= vertical > 0.0;
    }
    bounced.then_some(effect)
}

/// Multiplies the horizontal velocity by the speed factor of the blocks under the entity.
fn apply_speed_factor<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    mover: &Mover,
    body: &mut Body,
) {
    let factor = f64::from(speed_factor(world, table, body, mover.movement_efficiency));
    body.velocity.x *= factor;
    body.velocity.z *= factor;
}

/// The speed factor of the blocks under the body, mixed with `efficiency`.
pub(crate) fn speed_factor<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    body: &Body,
    efficiency: Option<f32>,
) -> f32 {
    let own = table.state(state_at(world, block_at(body.position)));
    let mut factor = own.motion.speed_factor;
    if !own.flags.contains(BlockFlags::OWN_SPEED_FACTOR) && factor == 1.0 {
        let below = offset_block(world, table, body, MOVEMENT_OFFSET);
        factor = table.state(state_at(world, below)).motion.speed_factor;
    }
    if let Some(efficiency) = efficiency {
        factor += efficiency * (1.0 - factor);
    }
    factor
}

/// The friction of the block under the body.
pub(crate) fn friction_below<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    body: &Body,
) -> f32 {
    let below = offset_block(world, table, body, MOVEMENT_OFFSET);
    table.state(state_at(world, below)).motion.friction
}

/// The block under the body for an offset, from its supporting block.
pub(crate) fn offset_block<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    body: &Body,
    offset: f32,
) -> BlockPos {
    let flags = body.support.map_or(BlockFlags::empty(), |at| {
        table.state(state_at(world, at)).flags
    });
    support::offset_block(body.support, flags, body.position, offset)
}
