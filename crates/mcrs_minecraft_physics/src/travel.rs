use glam::DVec3;

use crate::BlockFlags;
use crate::Body;
use crate::BodyView;
use crate::CollisionWorld;
use crate::MotionAttributes;
use crate::MotionEffects;
use crate::MoveInput;
use crate::MoveOutcome;
use crate::Mover;
use crate::MoverKind;
use crate::Scratch;
use crate::ShapeClass;
use crate::SineTable;
use crate::View;
use crate::block_scan::floor_block;
use crate::block_scan::state_at;
use crate::move_body;
use crate::move_body::offset_block;
use crate::sine_table::yaw_radians;
use crate::support::MOVEMENT_OFFSET;
use mcrs_voxel::BlockPos;

/// The largest horizontal speed and the fastest descent on a climbable block: `0.15F`.
const CLIMB_SPEED: f32 = 0.15;

/// The vertical velocity of a climb after a horizontal collision or a jump.
const CLIMB_UP: f64 = 0.2;

/// What the travel step of a living entity reads besides its body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TravelInput<'a> {
    pub(crate) mover: &'a Mover,
    pub(crate) attributes: &'a MotionAttributes,
    pub(crate) effects: &'a MotionEffects,
    pub(crate) input: MoveInput,
    /// The speed on the ground before the friction factor.
    pub(crate) speed: f32,
    /// The speed off the ground.
    pub(crate) air_speed: f32,
    pub(crate) yaw: f32,
    pub(crate) jump: bool,
    /// The type uses the horizontal air drag on the vertical axis too.
    pub(crate) omnidirectional: bool,
    /// The entity climbs in this tick as it did inside powder snow in the previous tick.
    pub(crate) climbs_powder_snow: bool,
    /// The gravity before the effect `slow_falling`.
    pub(crate) gravity: f64,
    pub(crate) player: bool,
}

/// The travel step of a living entity on land and in air. Returns the move.
pub(crate) fn travel<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    input: &TravelInput<'_>,
    body: &mut Body,
    scratch: &mut Scratch,
) -> MoveOutcome {
    let table = &view.tables.collision;
    let on_ground = body.flags.on_ground;
    let ground_friction = if on_ground {
        let below = offset_block(view.world, table, body, MOVEMENT_OFFSET);
        let block = table.state(state_at(view.world, below)).motion.friction;
        modifier(block, input.attributes.friction_modifier as f32)
    } else {
        1.0
    };
    let speed = if on_ground {
        if f64::from(ground_friction) > 0.6 {
            let cube = ground_friction * ground_friction * ground_friction;
            input.speed * (0.216_000_02_f32 / cube)
        } else {
            input.speed
        }
    } else {
        input.air_speed
    };
    accelerate(&view.tables.sine, body, input.input, speed, input.yaw);
    let climb = climbable(view, body);
    if let Some(block) = climb {
        let limit = f64::from(CLIMB_SPEED);
        body.velocity.x = body.velocity.x.clamp(-limit, limit);
        body.velocity.z = body.velocity.z.clamp(-limit, limit);
        body.velocity.y = body.velocity.y.max(-limit);
        if input.player && input.mover.shift && body.velocity.y < 0.0 && block != Climb::Scaffolding
        {
            body.velocity.y = 0.0;
        }
        body.fall_distance = 0.0;
    }
    let velocity = body.velocity;
    let slow_falling = input.effects.slow_falling;
    let mover = Mover {
        gravity: effective_gravity(input.gravity, slow_falling, velocity.y),
        ..*input.mover
    };
    let outcome = move_body::move_body(view, &mover, body, MoverKind::Own, velocity, scratch);
    let gravity = effective_gravity(input.gravity, slow_falling, body.velocity.y);
    if (climb.is_some() || input.climbs_powder_snow)
        && (body.flags.horizontal_collision || input.jump)
    {
        body.velocity.y = CLIMB_UP;
    }
    body.velocity.y = match input.effects.levitation {
        Some(level) => {
            let target = 0.05 * (f64::from(level) + 1.0);
            body.velocity.y + (target - body.velocity.y) * 0.2
        }
        None => body.velocity.y - gravity,
    };
    let air_drag = input.attributes.air_drag_modifier as f32;
    let horizontal_air = modifier(0.91, air_drag);
    let vertical_air = if input.omnidirectional {
        horizontal_air
    } else {
        modifier(0.98, air_drag)
    };
    let horizontal = f64::from(ground_friction * horizontal_air);
    body.velocity.x *= horizontal;
    body.velocity.z *= horizontal;
    body.velocity.y *= f64::from(vertical_air);
    outcome
}

/// The gravity after the effect `slow_falling`, which caps it at 0.01 while `vertical` is not
/// above zero.
pub(crate) fn effective_gravity(gravity: f64, slow_falling: bool, vertical: f64) -> f64 {
    if slow_falling && vertical <= 0.0 {
        gravity.min(0.01)
    } else {
        gravity
    }
}

/// `mod(f, k) = clamp(1 - (1 - f) × k, 0, 1)`, in f32.
pub(crate) fn modifier(factor: f32, modifier: f32) -> f32 {
    (1.0 - (1.0 - factor) * modifier).clamp(0.0, 1.0)
}

/// Adds the input vector, scaled by `speed` and rotated by the yaw, to the velocity.
pub(crate) fn accelerate(
    sine: &SineTable,
    body: &mut Body,
    input: MoveInput,
    speed: f32,
    yaw: f32,
) {
    let mut vector = DVec3::new(
        f64::from(input.sideways),
        f64::from(input.vertical),
        f64::from(input.forward),
    );
    let squared = vector.length_squared();
    if squared < 1.0e-7 {
        return;
    }
    if squared > 1.0 {
        let length = squared.sqrt();
        vector = if length < f64::from(1.0e-5_f32) {
            DVec3::ZERO
        } else {
            vector / length
        };
    }
    vector *= f64::from(speed);
    let angle = yaw_radians(yaw);
    let sin = f64::from(sine.sin(angle));
    let cos = f64::from(sine.cos(angle));
    body.velocity += DVec3::new(
        vector.x * cos - vector.z * sin,
        vector.y,
        vector.z * cos + vector.x * sin,
    );
}

/// The climbable block under the entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Climb {
    Scaffolding,
    Other,
}

fn climbable<W: CollisionWorld, B: BodyView>(view: View<'_, W, B>, body: &Body) -> Option<Climb> {
    let at = BlockPos::new(
        floor_block(body.position.x),
        floor_block(body.position.y),
        floor_block(body.position.z),
    );
    let block = view.tables.collision.state(state_at(view.world, at));
    if !block.flags.contains(BlockFlags::CLIMBABLE) {
        return None;
    }
    Some(match block.class {
        ShapeClass::Scaffolding { .. } => Climb::Scaffolding,
        _ => Climb::Other,
    })
}
