use glam::DVec3;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_numeric as numeric;
use mcrs_minecraft_pose::Dimensions;
use mcrs_minecraft_pose::Pose;

use crate::BlockAction;
use crate::BodyView;
use crate::CollisionWorld;
use crate::Keys;
use crate::MotionAttributes;
use crate::MoveInput;
use crate::MovePath;
use crate::Mover;
use crate::Orientation;
use crate::Physics;
use crate::PlayerFacts;
use crate::PlayerState;
use crate::PoseInputs;
use crate::Scratch;
use crate::Side;
use crate::StuckIgnore;
use crate::View;
use crate::block_effects;
use crate::block_effects::EffectContext;
use crate::push_out;
use crate::select_pose;
use crate::select_pose::pose_box_collides;
use crate::tick_entity::applied_dy;
use crate::tick_entity::jump;
use crate::tolerance::CLIP;
use crate::travel;
use crate::travel::TravelInput;

const DRAG: f32 = 0.98;
const JUMP_DELAY: u8 = 10;
const FLIGHT_WINDOW: u8 = 7;
const WORLD_EDGE: f64 = 2.999_999_9e7;
const FLIGHT_IMPULSE: f32 = 3.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    pub keys: Keys,
    pub rotation: Orientation,
    pub facts: PlayerFacts,
    pub push: DVec3,
}

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
        speed: movement_speed(out, sprint),
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
    let pose_inputs = PoseInputs {
        current: out.pose,
        entity: out.query_entity(),
        scale: out.attributes.scale as f32,
        sleeping: facts.sleeping,
        swimming: out.swimming,
        fall_flying: facts.fall_flying,
        spin_attack: facts.spin_attack,
        shift: keys.shift,
        flight: flight_end,
        spectator: facts.spectator,
        passenger: facts.passenger,
    };
    out.pose = select_pose(view, &pose_inputs, scratch);
    out.shift0 = keys.shift;
    out.jump0 = keys.jump;
    out.forward0 = move_input.forward;
    out.sprint0 = sprint;
    out.flight0 = flight_end;
}

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

pub(crate) fn player_mover(key: EntityUuid, side: Side, state: &PlayerState) -> Mover {
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

fn physics_of(facts: &PlayerFacts) -> Physics {
    if facts.spectator {
        Physics::Pass
    } else {
        Physics::Collide
    }
}

fn flight_impulse(keys: &Keys, flight_speed: f32) -> f64 {
    let direction = f64::from(i8::from(keys.jump) - i8::from(keys.shift));
    direction * f64::from(flight_speed * FLIGHT_IMPULSE)
}

pub(crate) fn zero_small_speeds(velocity: &mut DVec3) {
    if velocity.x * velocity.x + velocity.z * velocity.z < 9.0e-6 {
        velocity.x = 0.0;
        velocity.z = 0.0;
    }
    if velocity.y.abs() < 0.003 {
        velocity.y = 0.0;
    }
}

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

#[must_use]
pub fn dimensions_of(pose: Pose, attributes: &MotionAttributes) -> Dimensions {
    pose.player_dimensions().scaled(attributes.scale as f32)
}

fn movement_speed(state: &PlayerState, sprint: bool) -> f32 {
    let value = if sprint {
        state.sprint_speed.0
    } else {
        state.attributes.movement_speed
    };
    value as f32
}

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

fn intent(input: MoveInput, yaw: f32) -> DVec3 {
    let angle = numeric::radians_from_degrees(yaw);
    let sin = f64::from(numeric::sin_nearest(angle));
    let cos = f64::from(numeric::cos_nearest(angle));
    let x = f64::from(input.sideways);
    let z = f64::from(input.forward);
    DVec3::new(x * cos - z * sin, 0.0, z * cos + x * sin)
}

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

fn collides_in<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    state: &PlayerState,
    pose: Pose,
    scratch: &mut Scratch,
) -> bool {
    let dimensions = dimensions_of(pose, &state.attributes);
    pose_box_collides(view, &state.query_entity(), dimensions, scratch)
}
