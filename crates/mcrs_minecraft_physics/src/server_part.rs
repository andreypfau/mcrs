use glam::DVec3;

use crate::BlockAction;
use crate::BodyView;
use crate::CollisionWorld;
use crate::GridRow;
use crate::KeyedStream;
use crate::MovePath;
use crate::Mover;
use crate::Physics;
use crate::PlayerState;
use crate::PushOutcome;
use crate::PushRules;
use crate::Scratch;
use crate::Side;
use crate::StuckIgnore;
use crate::View;
use crate::block_effects;
use crate::block_effects::EffectContext;
use crate::push;
use crate::sine_table::yaw_radians;
use crate::tick_entity::jump_power;
use crate::tick_player::dimensions_of;
use crate::travel;

/// The decay of the velocity of the server copy of a player in each tick.
const DECAY: f64 = 0.98;

/// The values of the server part besides the state of the copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerCopy<'a> {
    /// The position before the moves of this tick: the start of the path through the blocks.
    pub start: DVec3,
    /// The player in the grid, in its state after the moves of this tick.
    pub row: &'a GridRow,
    pub rules: PushRules,
    pub stream: KeyedStream,
}

/// The server part of the tick of a player, on its server copy: the decay of the velocity, the
/// zero cutoff, the block effects without the fall distance, then the push of the
/// server-simulated entities near it.
pub fn server_part<B: BodyView, W: CollisionWorld>(
    view: View<'_, W, B>,
    state: &mut PlayerState,
    copy: &ServerCopy<'_>,
    actions: &mut Vec<BlockAction>,
    scratch: &mut Scratch,
) -> PushOutcome {
    let row = copy.row;
    state.velocity *= DECAY;
    let velocity = &mut state.velocity;
    if velocity.x * velocity.x + velocity.z * velocity.z < 9.0e-6 {
        velocity.x = 0.0;
        velocity.z = 0.0;
    }
    if velocity.y.abs() < 0.003 {
        velocity.y = 0.0;
    }
    let attributes = state.attributes;
    let dimensions = dimensions_of(state.pose, &attributes);
    let mover = Mover {
        key: row.key,
        width: dimensions.width,
        height: dimensions.height,
        step_height: attributes.step_height as f32,
        physics: Physics::Collide,
        side: Side::Server { authority: false },
        shift: state.shift0,
        keeps_to_edges: !state.flight0,
        restitution: attributes.bounciness,
        block_bounce_factor: 1.0,
        gravity: attributes.gravity,
        vertical_drag: travel::modifier(0.98, attributes.air_drag_modifier as f32),
        movement_efficiency: Some(attributes.movement_efficiency as f32),
        powder_snow_solid: false,
        walks_on_powder_snow: false,
        intent: None,
    };
    let context = EffectContext {
        mover: &mover,
        stuck_ignore: StuckIgnore::None,
        powder_snow_needs_inside: true,
        path: MovePath {
            start: copy.start,
            end: state.position,
        },
        dy: 0.0,
        flying: state.flight0,
        updates_fall: false,
    };
    let mut body = state.body();
    block_effects::apply(view, &context, &mut body, None, actions);
    state.set_body(&body);
    push::push_entity(
        view.bodies,
        row,
        &copy.rules,
        &copy.stream,
        &mut state.velocity,
        scratch,
    )
}

/// The jump of the server copy when the server accepts a move that goes up from the ground.
pub fn server_jump<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    state: &mut PlayerState,
    yaw: f32,
) {
    let body = state.body();
    let Some(power) = jump_power(
        view,
        &body,
        state.attributes.jump_strength,
        state.effects.jump_boost,
    ) else {
        return;
    };
    state.velocity.y = power.max(state.velocity.y);
    if state.sprint0 {
        let angle = yaw_radians(yaw);
        let sine = &view.tables.sine;
        state.velocity += DVec3::new(
            -f64::from(sine.sin(angle)) * 0.2,
            0.0,
            f64::from(sine.cos(angle)) * 0.2,
        );
    }
}
