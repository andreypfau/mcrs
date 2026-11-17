use glam::DVec3;
use mcrs_minecraft_entity_key::KeyedStream;

use crate::BlockAction;
use crate::BodyView;
use crate::CollisionWorld;
use crate::GridRow;
use crate::MovePath;
use crate::Mover;
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
use crate::tick_entity::jump;
use crate::tick_player::player_mover;
use crate::tick_player::zero_small_speeds;

const DECAY: f64 = 0.98;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerCopy<'a> {
    pub start: DVec3,
    pub row: &'a GridRow,
    pub rules: PushRules,
    pub stream: KeyedStream,
}

pub fn server_part<B: BodyView, W: CollisionWorld>(
    view: View<'_, W, B>,
    state: &mut PlayerState,
    copy: &ServerCopy<'_>,
    actions: &mut Vec<BlockAction>,
    scratch: &mut Scratch,
) -> PushOutcome {
    let row = copy.row;
    state.body.velocity *= DECAY;
    zero_small_speeds(&mut state.body.velocity);
    let mover = Mover {
        shift: state.shift0,
        keeps_to_edges: !state.flight0,
        ..player_mover(row.key, Side::Server { authority: false }, state)
    };
    let context = EffectContext {
        mover: &mover,
        stuck_ignore: StuckIgnore::None,
        powder_snow_needs_inside: true,
        path: MovePath {
            start: copy.start,
            end: state.body.position,
        },
        dy: 0.0,
        flying: state.flight0,
        updates_fall: false,
        weaving: state.effects.weaving,
    };
    let mut body = state.body;
    block_effects::apply(view, &context, &mut body, None, actions);
    state.body = body;
    push::push_entity(
        view.bodies,
        row,
        &copy.rules,
        &copy.stream,
        &mut state.body.velocity,
        scratch,
    )
}

pub fn server_jump<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    state: &mut PlayerState,
    yaw: f32,
) {
    let mut body = state.body;
    let attributes = &state.attributes;
    let sprint_yaw = state.sprint0.then_some(yaw);
    jump(
        view,
        &mut body,
        attributes.jump_strength,
        state.effects.jump_boost,
        sprint_yaw,
    );
    state.body = body;
}
