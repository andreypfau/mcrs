use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::Aabb;
use crate::BirthKey;
use crate::BlockAction;
use crate::Body;
use crate::BodyView;
use crate::Category;
use crate::CollisionWorld;
use crate::Dimensions;
use crate::EntityState;
use crate::Living;
use crate::MotionRow;
use crate::MovePath;
use crate::Mover;
use crate::MoverKind;
use crate::Physics;
use crate::Rest;
use crate::Scratch;
use crate::Side;
use crate::TickOutcome;
use crate::Travel;
use crate::View;
use crate::air_batch;
use crate::block_effects;
use crate::block_effects::EffectContext;
use crate::block_scan::floor_block;
use crate::block_scan::state_at;
use crate::move_body::move_body;
use crate::move_body::offset_block;
use crate::query;
use crate::rest::same_bits;
use crate::support::MOVEMENT_OFFSET;
use crate::tolerance::CLIP;
use crate::tolerance::COLLISION;
use crate::travel;
use crate::travel::TravelInput;

const DRAG: f32 = 0.98;
const GROUND_BOUNCE: DVec3 = DVec3::new(0.7, -0.5, 0.7);
const SMALL_SPEED: f64 = 0.003;
const JUMP_DELAY: u8 = 10;
const AIR_SPEED: f32 = 0.02;

/// One tick of the physics part of a server-simulated entity that is not a player. `wake` is
/// true when something in the read zone changed, or a write changed the entity, since its last
/// tick: a resting entity then runs a full tick.
pub fn tick_entity<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    side: Side,
    row: &MotionRow,
    entity: &mut EntityState,
    wake: bool,
    actions: &mut Vec<BlockAction>,
    scratch: &mut Scratch,
) -> TickOutcome {
    let cycle = row.category != Category::Item || item_moves(entity);
    if wake {
        entity.rest = Rest::Awake;
    }
    if cycle && let Rest::Resting(state) = entity.rest {
        entity.body = state;
        return TickOutcome {
            fixed_point: true,
            skipped: true,
            ..TickOutcome::default()
        };
    }
    let before = entity.body;
    let first_action = actions.len();
    let mut outcome = match row.category {
        Category::Walker | Category::Omnidirectional => {
            tick_living(view, side, row, entity, actions, scratch)
        }
        Category::Item | Category::FallingBlock | Category::PrimedTnt | Category::ExperienceOrb => {
            tick_other(view, side, row, entity, actions, scratch)
        }
    };
    if cycle {
        let quiet = actions.len() == first_action
            && entity.body.stuck == DVec3::ZERO
            && entity.living.is_none_or(|living| is_idle(&living));
        let reference = match (row.category, entity.rest) {
            (Category::Item, Rest::Candidate(state) | Rest::Resting(state)) => Some(state),
            (Category::Item, Rest::Awake) => None,
            _ => Some(before),
        };
        outcome.fixed_point =
            quiet && reference.is_some_and(|state| same_bits(&state, &entity.body));
        entity.rest = if outcome.fixed_point {
            Rest::Resting(entity.body)
        } else if row.category == Category::Item {
            Rest::Candidate(entity.body)
        } else {
            Rest::Awake
        };
    }
    outcome
}

/// The box of the read zone of a tick: the box and the box moved by `displacement`, grown by 1
/// on `x` and `z` and by `1 + step_height` up and down.
#[must_use]
pub fn read_zone(bounds: Aabb, displacement: DVec3, step_height: f32) -> Aabb {
    let moved = bounds.translate(displacement);
    let union = Aabb::new(bounds.min.min(moved.min), bounds.max.max(moved.max));
    union.inflate(DVec3::new(1.0, 1.0 + f64::from(step_height), 1.0))
}

fn is_idle(living: &Living) -> bool {
    living.input.length_squared() < 1.0e-7
        && !living.jump
        && living.jump_delay == 0
        && living
            .steering
            .is_none_or(|steering| steering.input.length_squared() < 1.0e-7 && !steering.jump)
}

fn tick_living<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    side: Side,
    row: &MotionRow,
    entity: &mut EntityState,
    actions: &mut Vec<BlockAction>,
    scratch: &mut Scratch,
) -> TickOutcome {
    let Some(mut living) = entity.living else {
        return TickOutcome::default();
    };
    let body = &mut entity.body;
    living.jump_delay = living.jump_delay.saturating_sub(1);
    for component in [
        &mut body.velocity.x,
        &mut body.velocity.y,
        &mut body.velocity.z,
    ] {
        if component.abs() < SMALL_SPEED {
            *component = 0.0;
        }
    }
    living.input.sideways *= DRAG;
    living.input.forward *= DRAG;
    if let Some(steering) = living.steering {
        living.input = steering.input;
        living.jump = steering.jump;
        living.speed = steering.speed;
    }
    let attributes = living.attributes;
    let dimensions = row.dimensions.scaled(attributes.scale as f32);
    let omnidirectional = row.category == Category::Omnidirectional;
    let air_drag = attributes.air_drag_modifier as f32;
    let vertical_drag = if omnidirectional {
        travel::modifier(0.91, air_drag)
    } else {
        travel::modifier(DRAG, air_drag)
    };
    let mut mover = living_mover(entity.key, side, row, &living, vertical_drag, dimensions);
    if !living.jump {
        living.jump_delay = 0;
    } else if body.flags.on_ground && living.jump_delay == 0 {
        jump(view, body, &living);
        living.jump_delay = JUMP_DELAY;
    }
    let effects = living.effects;
    if effects.slow_falling || effects.levitation.is_some() {
        body.fall_distance = 0.0;
    }
    let start = body.position;
    let mut dy = 0.0;
    if living.travel == Travel::Open {
        let base = if entity.no_gravity {
            0.0
        } else {
            attributes.gravity
        };
        mover.gravity = if effects.slow_falling && body.velocity.y <= 0.0 {
            base.min(0.01)
        } else {
            base
        };
        let input = TravelInput {
            mover: &mover,
            attributes: &attributes,
            effects: &effects,
            input: living.input,
            speed: living.speed,
            air_speed: AIR_SPEED,
            yaw: living.yaw,
            jump: living.jump,
            omnidirectional,
            climbs_powder_snow: living.in_powder_snow && living.walks_on_powder_snow,
            gravity: mover.gravity,
            player: false,
        };
        let moved = travel::travel(view, &input, body, scratch);
        dy = applied_dy(&moved.resolved, start, body.position);
    }
    let context = EffectContext {
        mover: &mover,
        stuck_ignore: row.stuck_ignore,
        powder_snow_needs_inside: row.powder_snow_needs_inside,
        path: MovePath {
            start,
            end: body.position,
        },
        dy,
        flying: false,
        updates_fall: true,
    };
    block_effects::apply(view, &context, body, Some(&mut living), actions);
    entity.living = Some(living);
    TickOutcome::default()
}

/// The mover of a living entity that is not a player; the travel step sets its gravity.
fn living_mover(
    key: BirthKey,
    side: Side,
    row: &MotionRow,
    living: &Living,
    vertical_drag: f32,
    dimensions: Dimensions,
) -> Mover {
    let attributes = &living.attributes;
    Mover {
        key,
        width: dimensions.width,
        height: dimensions.height,
        step_height: attributes.step_height as f32,
        physics: Physics::Collide,
        side,
        shift: false,
        keeps_to_edges: false,
        restitution: attributes.bounciness,
        block_bounce_factor: row.block_bounce_factor,
        gravity: 0.0,
        vertical_drag,
        movement_efficiency: Some(attributes.movement_efficiency as f32),
        powder_snow_solid: row.powder_snow_solid,
        walks_on_powder_snow: living.walks_on_powder_snow,
        intent: None,
    }
}

/// Sets the vertical velocity of a jump from the ground.
fn jump<W: CollisionWorld, B: BodyView>(view: View<'_, W, B>, body: &mut Body, living: &Living) {
    let attributes = &living.attributes;
    if let Some(power) = jump_power(
        view,
        body,
        attributes.jump_strength,
        living.effects.jump_boost,
    ) {
        body.velocity.y = power.max(body.velocity.y);
    }
}

/// The widened jump power from the jump factor of the blocks under the body, or `None` when it
/// is not more than `1.0E-5F`.
pub(crate) fn jump_power<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    body: &Body,
    jump_strength: f64,
    jump_boost: Option<u8>,
) -> Option<f64> {
    let table = &view.tables.collision;
    let floor = BlockPos::new(
        floor_block(body.position.x),
        floor_block(body.position.y),
        floor_block(body.position.z),
    );
    let mut factor = table.state(state_at(view.world, floor)).motion.jump_factor;
    if factor == 1.0 {
        let below = offset_block(view.world, table, body, MOVEMENT_OFFSET);
        factor = table.state(state_at(view.world, below)).motion.jump_factor;
    }
    let boost = jump_boost.map_or(0.0, |level| 0.1_f32 * (f32::from(level) + 1.0));
    let power = (jump_strength as f32) * 1.0 * factor + boost;
    (f64::from(power) > CLIP).then_some(f64::from(power))
}

fn tick_other<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    side: Side,
    row: &MotionRow,
    entity: &mut EntityState,
    actions: &mut Vec<BlockAction>,
    scratch: &mut Scratch,
) -> TickOutcome {
    if side.has_authority() && air_batch::in_open_air(view, row, entity, &mut scratch.entities) {
        air_batch::air_step(row, entity);
        return TickOutcome::default();
    }
    let gravity = if entity.no_gravity { 0.0 } else { row.gravity };

    let mut mover = Mover {
        key: entity.key,
        width: row.dimensions.width,
        height: row.dimensions.height,
        step_height: 0.0,
        physics: Physics::Collide,
        side,
        shift: false,
        keeps_to_edges: false,
        restitution: 0.0,
        block_bounce_factor: row.block_bounce_factor,
        gravity,
        vertical_drag: DRAG,
        movement_efficiency: None,
        powder_snow_solid: row.powder_snow_solid,
        walks_on_powder_snow: false,
        intent: None,
    };
    let mut outcome = TickOutcome::default();
    let moves = item_moves(entity);
    let body = &mut entity.body;
    let start = body.position;
    let category = row.category;
    let resting_item = category == Category::Item
        && body.flags.on_ground
        && body.velocity.x * body.velocity.x + body.velocity.z * body.velocity.z <= CLIP
        && !moves;
    let falls = match category {
        Category::ExperienceOrb => {
            let bounds = Aabb::of_body(body.position, mover.width, mover.height);
            !query::intersects(
                view,
                &mover.query_entity(body),
                bounds,
                &mut scratch.entities,
            )
        }
        _ => true,
    };
    if falls {
        body.velocity.y -= gravity;
    }
    let fall_velocity = body.velocity.y;
    let mut dy = 0.0;
    if !resting_item {
        if category == Category::Item && side.has_authority() {
            let bounds = Aabb::of_body(body.position, mover.width, mover.height).deflate(COLLISION);
            if query::intersects(
                view,
                &mover.query_entity(body),
                bounds,
                &mut scratch.entities,
            ) {
                mover.physics = Physics::Pass;
                outcome.stuck_in_block = true;
            }
        }
        let velocity = body.velocity;
        let moved = move_body(view, &mover, body, MoverKind::Own, velocity, scratch);
        dy = applied_dy(&moved.resolved, start, body.position);
        entity.last_path = MovePath {
            start,
            end: body.position,
        };
    }
    let context = EffectContext {
        mover: &mover,
        stuck_ignore: row.stuck_ignore,
        powder_snow_needs_inside: row.powder_snow_needs_inside,
        path: entity.last_path,
        dy,
        flying: false,
        updates_fall: true,
    };
    block_effects::apply(view, &context, body, None, actions);
    if resting_item {
        return outcome;
    }
    if category == Category::FallingBlock && body.flags.on_ground && side.has_authority() {
        body.velocity *= GROUND_BOUNCE;
        outcome.landed = true;
    }
    drag_and_ground_rule(view, category, body, fall_velocity, gravity);
    outcome
}

/// The drag of a category that is not living, with the bounce of an item and an orb and the
/// ground rule of a primed TNT.
fn drag_and_ground_rule<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    category: Category,
    body: &mut Body,
    fall_velocity: f64,
    gravity: f64,
) {
    let drag = f64::from(DRAG);
    let ground = if body.flags.on_ground {
        let below = offset_block(view.world, &view.tables.collision, body, MOVEMENT_OFFSET);
        let friction = view
            .tables
            .collision
            .state(state_at(view.world, below))
            .motion
            .friction;
        f64::from(DRAG * friction)
    } else {
        drag
    };
    match category {
        Category::Item => {
            body.velocity *= DVec3::new(ground, drag, ground);
            if body.flags.on_ground && body.velocity.y < 0.0 {
                body.velocity.y *= -0.5;
            }
        }
        Category::ExperienceOrb => {
            body.velocity *= ground;
            if body.flags.below_collision && fall_velocity < -gravity {
                body.velocity.y = -0.4 * fall_velocity;
            }
        }
        Category::PrimedTnt => {
            body.velocity *= drag;
            if body.flags.on_ground {
                body.velocity *= GROUND_BOUNCE;
            }
        }
        Category::FallingBlock => body.velocity *= drag,
        Category::Walker | Category::Omnidirectional => {}
    }
}

/// True in the tick of the rest cycle of 4 in which an item on the ground moves.
fn item_moves(entity: &EntityState) -> bool {
    entity
        .age
        .wrapping_add(u32::from(entity.phase))
        .is_multiple_of(4)
}

/// The vertical displacement of the move, or zero when the move left the position as it was.
fn applied_dy(resolved: &DVec3, start: DVec3, end: DVec3) -> f64 {
    if start == end { 0.0 } else { resolved.y }
}
