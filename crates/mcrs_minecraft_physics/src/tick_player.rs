use glam::DVec3;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_numeric as numeric;

use crate::Aabb;
use crate::BlockAction;
use crate::BodyView;
use crate::CollisionWorld;
use crate::Dimensions;
use crate::Keys;
use crate::MotionAttributes;
use crate::MoveInput;
use crate::MovePath;
use crate::Mover;
use crate::Orientation;
use crate::Physics;
use crate::PlayerFacts;
use crate::PlayerState;
use crate::Pose;
use crate::Scratch;
use crate::Side;
use crate::StuckIgnore;
use crate::View;
use crate::block_effects;
use crate::block_effects::EffectContext;
use crate::push_out;
use crate::query;
use crate::tick_entity::applied_dy;
use crate::tick_entity::jump;
use crate::tolerance::CLIP;
use crate::tolerance::COLLISION;
use crate::travel;
use crate::travel::TravelInput;

const DRAG: f32 = 0.98;
const JUMP_DELAY: u8 = 10;
/// The ticks after a press of the jump key in which a second press toggles the flight state.
const FLIGHT_WINDOW: u8 = 7;
const WORLD_EDGE: f64 = 2.999_999_9e7;
/// The modifier of the sprint state on `movement_speed`, with the operation "multiply total".
const SPRINT_BOOST: f32 = 0.3;
const FLIGHT_IMPULSE: f32 = 3.0;

/// What one tick of a player reads besides its state and the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    pub keys: Keys,
    /// The rotation of this tick; it applies before the tick.
    pub rotation: Orientation,
    pub facts: PlayerFacts,
    /// The push of other entities since the last tick; it adds to the velocity before the tick.
    pub push: DVec3,
}

/// One tick of a player on the side that simulates it, from `start` into `out`.
pub fn tick_player<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    side: Side,
    start: &PlayerState,
    input: &PlayerInput,
    out: &mut PlayerState,
    actions: &mut Vec<BlockAction>,
    scratch: &mut Scratch,
) {
    *out = *start;
    out.body.velocity += input.push;
    let keys = input.keys;
    let facts = input.facts;
    let yaw = input.rotation.yaw;
    if facts.spectator || facts.passenger {
        out.body.flags.on_ground = false;
    }
    out.sprint_timer = out.sprint_timer.saturating_sub(1);
    out.crouching = crouch_state(view, start, &facts, scratch);
    let move_input = input_vector(&keys, out, &facts);
    if !facts.spectator && !facts.passenger {
        push_out::push_out_of_blocks(view.world, &view.tables.collision, out);
    }
    let sprint = sprint_state(start, out, &keys, &facts, move_input.forward);
    let flight = flight_steps(view, side, start, out, input, sprint);
    out.jump_delay = out.jump_delay.saturating_sub(1);
    zero_small_speeds(&mut out.body.velocity);
    let attributes = out.attributes;
    let mut body = out.body;
    player_jump(view, out, &mut body, &keys, flight, sprint, yaw);
    if out.effects.slow_falling || out.effects.levitation.is_some() {
        body.fall_distance = 0.0;
    }
    let mover = Mover {
        physics: physics_of(&facts),
        shift: keys.shift,
        keeps_to_edges: !flight,
        intent: (side == Side::OwnClient).then(|| intent(move_input, yaw)),
        ..player_mover(out.key, side, out)
    };
    let start_position = body.position;
    let air_speed = air_speed(flight, sprint, out.flight_speed);
    let travel_input = TravelInput {
        mover: &mover,
        attributes: &attributes,
        effects: &out.effects,
        input: move_input,
        speed: movement_speed(&attributes, sprint),
        air_speed,
        yaw,
        jump: keys.jump,
        omnidirectional: false,
        climbs_powder_snow: false,
        player: true,
    };
    let kept_y = body.velocity.y;
    let moved = travel::travel(view, &travel_input, &mut body, scratch);
    if flight && !facts.passenger {
        body.velocity.y = kept_y * 0.6;
    }
    let dy = applied_dy(&moved.resolved, start_position, body.position);
    let context = EffectContext {
        mover: &mover,
        stuck_ignore: StuckIgnore::None,
        powder_snow_needs_inside: true,
        path: MovePath {
            start: start_position,
            end: body.position,
        },
        dy,
        flying: flight,
        updates_fall: true,
        weaving: out.effects.weaving,
    };
    block_effects::apply(view, &context, &mut body, None, actions);
    let flight_end = flight && !(body.flags.on_ground && !facts.spectator);
    body.position.x = body.position.x.clamp(-WORLD_EDGE, WORLD_EDGE);
    body.position.z = body.position.z.clamp(-WORLD_EDGE, WORLD_EDGE);
    out.body = body;
    out.pose = select_pose(view, out, &keys, &facts, flight_end, scratch);
    out.shift0 = keys.shift;
    out.jump0 = keys.jump;
    out.forward0 = move_input.forward;
    out.sprint0 = sprint;
    out.flight0 = flight_end;
}

/// Steps 6 to 8 of the tick for the flight state: the toggle, the jump when the flight starts on
/// the ground, the vertical impulse and the reset of the fall distance. Returns the flight state.
fn flight_steps<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    side: Side,
    start: &PlayerState,
    out: &mut PlayerState,
    input: &PlayerInput,
    sprint: bool,
) -> bool {
    let flight = flight_state(start, out, &input.keys, &input.facts);
    if flight && !start.flight0 && out.body.flags.on_ground {
        let mut body = out.body;
        let sprint_yaw = sprint.then_some(input.rotation.yaw);
        let strength = out.attributes.jump_strength;
        jump(
            view,
            &mut body,
            strength,
            out.effects.jump_boost,
            sprint_yaw,
        );
        out.body = body;
    }
    if flight && side == Side::OwnClient {
        out.body.velocity.y += flight_impulse(&input.keys, out.flight_speed);
    }
    out.jump_timer = out.jump_timer.saturating_sub(1);
    if flight {
        out.body.fall_distance = 0.0;
    }
    flight
}

/// The mover of a player in its pose and with its attributes.
pub(crate) fn player_mover(key: EntityKey, side: Side, state: &PlayerState) -> Mover {
    let attributes = &state.attributes;
    Mover {
        step_height: attributes.step_height as f32,
        restitution: attributes.bounciness,
        gravity: attributes.gravity,
        vertical_drag: travel::modifier(DRAG, attributes.air_drag_modifier as f32),
        movement_efficiency: Some(attributes.movement_efficiency as f32),
        ..Mover::new(key, dimensions_of(state.pose, attributes), side)
    }
}

/// A spectator has no physics.
fn physics_of(facts: &PlayerFacts) -> Physics {
    if facts.spectator {
        Physics::Pass
    } else {
        Physics::Collide
    }
}

/// The vertical impulse of a flying player from the jump and shift keys.
fn flight_impulse(keys: &Keys, flight_speed: f32) -> f64 {
    let direction = f64::from(i8::from(keys.jump) - i8::from(keys.shift));
    direction * f64::from(flight_speed * FLIGHT_IMPULSE)
}

/// Sets a horizontal speed below `0.003` and a small vertical speed to zero.
pub(crate) fn zero_small_speeds(velocity: &mut DVec3) {
    if velocity.x * velocity.x + velocity.z * velocity.z < 9.0e-6 {
        velocity.x = 0.0;
        velocity.z = 0.0;
    }
    if velocity.y.abs() < 0.003 {
        velocity.y = 0.0;
    }
}

/// The jump of PHY-INP-004 and PHY-INP-005: the jump delay gates the jump from the ground.
fn player_jump<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    out: &mut PlayerState,
    body: &mut crate::Body,
    keys: &Keys,
    flight: bool,
    sprint: bool,
    yaw: f32,
) {
    if !keys.jump || flight {
        out.jump_delay = 0;
        return;
    }
    if !body.flags.on_ground || out.jump_delay != 0 {
        return;
    }
    let strength = out.attributes.jump_strength;
    jump(
        view,
        body,
        strength,
        out.effects.jump_boost,
        sprint.then_some(yaw),
    );
    out.jump_delay = JUMP_DELAY;
}

/// The speed of the input acceleration off the ground.
fn air_speed(flight: bool, sprint: bool, flight_speed: f32) -> f32 {
    if flight {
        if sprint {
            flight_speed * 2.0
        } else {
            flight_speed
        }
    } else if sprint {
        0.025_999_999
    } else {
        0.02
    }
}

/// The dimensions of a player in a pose, scaled by the `scale` attribute.
#[must_use]
pub fn dimensions_of(pose: Pose, attributes: &MotionAttributes) -> Dimensions {
    pose.player_dimensions().scaled(attributes.scale as f32)
}

/// `f32(movement_speed)`, with the sprint modifier when the sprint state is on.
fn movement_speed(attributes: &MotionAttributes, sprint: bool) -> f32 {
    let mut value = attributes.movement_speed;
    if sprint {
        value *= 1.0 + f64::from(SPRINT_BOOST);
    }
    value.clamp(0.0, 1024.0) as f32
}

/// The input vector from the keys, all in f32.
fn input_vector(keys: &Keys, state: &PlayerState, facts: &PlayerFacts) -> MoveInput {
    let axis = |positive: bool, negative: bool| f32::from(i8::from(positive) - i8::from(negative));
    let mut left = axis(keys.left, keys.right);
    let mut forward = axis(keys.forward, keys.back);
    let length = (left * left + forward * forward).sqrt();
    if length < 1.0e-4 {
        left = 0.0;
        forward = 0.0;
    } else {
        left /= length;
        forward /= length;
        left *= DRAG;
        forward *= DRAG;
    }
    if state.item_use.using && !facts.passenger {
        left *= state.item_use.speed_multiplier;
        forward *= state.item_use.speed_multiplier;
    }
    let crawling = state.pose == Pose::Swimming && !state.swimming;
    if state.crouching || crawling {
        let factor = state.attributes.sneaking_speed as f32;
        left *= factor;
        forward *= factor;
    }
    let length = (left * left + forward * forward).sqrt();
    if length > 0.0 {
        let scale = 1.0 / length;
        let (unit_left, unit_forward) = (left * scale, forward * scale);
        let (small, large) = if unit_left.abs() < unit_forward.abs() {
            (unit_left.abs(), unit_forward.abs())
        } else {
            (unit_forward.abs(), unit_left.abs())
        };
        let ratio = small / large;
        let target = (length * (1.0 + ratio * ratio).sqrt()).min(1.0);
        left = unit_left * target;
        forward = unit_forward * target;
    }
    MoveInput {
        sideways: left,
        vertical: 0.0,
        forward,
    }
}

/// The intended horizontal direction for the minor collision flag: the input rotated by the yaw,
/// with the table values widened before the products.
fn intent(input: MoveInput, yaw: f32) -> DVec3 {
    let angle = numeric::radians_from_degrees(yaw);
    let sin = f64::from(numeric::sin_nearest(angle));
    let cos = f64::from(numeric::cos_nearest(angle));
    let x = f64::from(input.sideways);
    let z = f64::from(input.forward);
    DVec3::new(x * cos - z * sin, 0.0, z * cos + x * sin)
}

/// The crouch state of this tick, from the shift key and the flight state of the previous tick.
fn crouch_state<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    start: &PlayerState,
    facts: &PlayerFacts,
    scratch: &mut Scratch,
) -> bool {
    if start.flight0 || start.swimming || facts.passenger {
        return false;
    }
    if collides_in(view, start, Pose::Crouching, scratch) {
        return false;
    }
    start.shift0 || (!facts.sleeping && collides_in(view, start, Pose::Standing, scratch))
}

/// The sprint state after step 6 of the tick.
fn sprint_state(
    start: &PlayerState,
    out: &mut PlayerState,
    keys: &Keys,
    facts: &PlayerFacts,
    forward: f32,
) -> bool {
    let forward_before = start.forward0 > CLIP as f32;
    if start.shift0 || keys.back || out.item_use.slows() {
        out.sprint_timer = 0;
    }
    let crawling = out.pose == Pose::Swimming && !out.swimming;
    let allowed = !out.effects.blindness && (facts.fed || out.may_fly);
    let moving = forward > CLIP as f32;
    let mut sprint = start.sprint0;
    let may_start =
        !sprint && moving && allowed && !out.item_use.slows() && !out.crouching && !crawling;
    if may_start {
        if !forward_before {
            if out.sprint_timer > 0 {
                sprint = true;
            } else {
                out.sprint_timer = facts.sprint_window;
            }
        }
        if keys.sprint {
            sprint = true;
        }
    }
    if sprint && !out.swimming {
        let blocked = start.body.flags.horizontal_collision && !start.body.flags.minor_collision;
        if !allowed || !moving || blocked {
            sprint = false;
        }
    }
    sprint
}

/// The flight state after step 6 of the tick.
fn flight_state(
    start: &PlayerState,
    out: &mut PlayerState,
    keys: &Keys,
    facts: &PlayerFacts,
) -> bool {
    if facts.spectator {
        return true;
    }
    let mut flight = start.flight0;
    let pressed = keys.jump && !start.jump0 && !keys.auto_jump;
    let may_toggle = out.may_fly && !out.swimming && (!facts.passenger || facts.vehicle_takes_jump);
    if may_toggle && pressed {
        if out.jump_timer > 0 {
            flight = !flight;
            out.jump_timer = 0;
        } else {
            out.jump_timer = FLIGHT_WINDOW;
        }
    }
    flight
}

/// The pose at the end of the tick.
fn select_pose<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    state: &PlayerState,
    keys: &Keys,
    facts: &PlayerFacts,
    flight: bool,
    scratch: &mut Scratch,
) -> Pose {
    let desired = if facts.sleeping {
        Pose::Sleeping
    } else if state.swimming {
        Pose::Swimming
    } else if keys.shift && !flight {
        Pose::Crouching
    } else {
        Pose::Standing
    };
    if collides_in(view, state, Pose::Swimming, scratch) {
        return state.pose;
    }
    if facts.spectator || facts.passenger || !collides_in(view, state, desired, scratch) {
        return desired;
    }
    if !collides_in(view, state, Pose::Crouching, scratch) {
        return Pose::Crouching;
    }
    Pose::Swimming
}

/// True when the box of the pose, shrunk by `1.0E-7`, meets a collider.
fn collides_in<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    state: &PlayerState,
    pose: Pose,
    scratch: &mut Scratch,
) -> bool {
    let dimensions = dimensions_of(pose, &state.attributes);
    let bounds =
        Aabb::of_body(state.body.position, dimensions.width, dimensions.height).deflate(COLLISION);
    query::intersects(view, &state.query_entity(), bounds, &mut scratch.entities)
}
