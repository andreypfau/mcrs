use crate::ability::PlayerGameMode;
use crate::block_break::{
    DESTRUCTION_NONE, Hit, break_block, destruction, eye_position, harvest, level_event, observed,
};
use crate::break_session::{BreakSession, Crack};
use crate::bus::OutboundPlayerPacket;
use crate::client_on_ground::ClientOnGround;
use crate::column_change::ColumnChange;
use crate::dig_rate::dig_rate;
use crate::diggers::Diggers;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::{Query, Res, ResMut};
use mcrs_minecraft_block::definition::{BlockStateFlags, Blocks};
use mcrs_minecraft_engine_world::block_drops::BlockBroken;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_volume::{BlockVolume, Unavailable};
use mcrs_voxel::{BlockPos, BlockStateId};

const HIT_EVENT: i32 = 2019;
const FOURTH_HIT_EVENT: i32 = 2020;

pub(crate) fn any_digger(diggers: Res<Diggers>) -> bool {
    !diggers.0.is_empty()
}

fn advance(
    crack: &mut Crack,
    rate: f32,
    breaker: PlayerSession,
    changes: &mut MessageWriter<ColumnChange>,
) {
    crack.dig.advance(rate);
    let stage = crack.dig.stage();
    if stage != crack.sent_stage {
        crack.sent_stage = stage;
        changes.write(destruction(breaker, crack.pos, stage.cast_signed()));
    }
}

/// One tick of each dig and of each delayed break. It runs after the requests of the tick, so
/// the tick of a start action is the first tick of its dig.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query, writer and resource is its own parameter"
)]
pub(crate) fn advance_digs(
    mut diggers: ResMut<Diggers>,
    mut players: Query<(
        &Owner,
        &PlayerGameMode,
        &InDimension,
        &Transform,
        &ClientOnGround,
        &mut BreakSession,
    )>,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut changes: MessageWriter<ColumnChange>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut broken_blocks: MessageWriter<BlockBroken>,
) {
    let Some(definitions) = definitions else {
        return;
    };
    diggers.0.retain(|&player| {
        // chisle: a dig does not cross a cell or a dimension change: the observers keep its crack
        // and a later finish action is refused; carrying the session with the player lifts it.
        let Ok((&Owner(session), mode, dimension, transform, &on_ground, mut digging)) =
            players.get_mut(player)
        else {
            return false;
        };
        let view = world
            .get(dimension.0)
            .filter(|_| mode.0 == GameMode::Survival);
        let eye = eye_position(transform.translation);
        // `Err` is a block that this tick cannot read: its dig waits, it does not end.
        let diggable = |pos: BlockPos| -> Result<Option<(BlockStateId, f32)>, Unavailable> {
            let Some(view) = view.as_ref() else {
                return Ok(None);
            };
            let state = match view.block(pos) {
                Ok(state) => state,
                Err(Unavailable::Outside) => return Ok(None),
                Err(unreadable) => return Err(unreadable),
            };
            let air = definitions
                .state(state)
                .flags
                .contains(BlockStateFlags::IS_AIR);
            Ok((!air).then(|| (state, dig_rate(&definitions, view, state, eye, on_ground))))
        };
        // chisle: every holder of the column gets the stages and the hit events; a radius in the
        // delivery of a column change lifts it to the limits of 32 and 64 blocks.
        if let Some(mut dig) = digging.dig.take() {
            let pos = dig.crack.pos;
            match diggable(pos) {
                Ok(Some((_, rate))) => {
                    advance(&mut dig.crack, rate, session, &mut changes);
                    if dig.crack.dig.progress() <= 1.0 {
                        let effect = if dig.crack.dig.ticks().is_multiple_of(4) {
                            FOURTH_HIT_EVENT
                        } else {
                            HIT_EVENT
                        };
                        let face = dig.face.id() as i32;
                        changes.write(observed(pos, level_event(effect, pos, face), None));
                    }
                    digging.dig = Some(dig);
                }
                Ok(None) => {
                    changes.write(destruction(session, pos, DESTRUCTION_NONE));
                }
                Err(_) => digging.dig = Some(dig),
            }
        }
        if let Some(mut delayed) = digging.delayed.take() {
            let pos = delayed.crack.pos;
            let found = diggable(pos);
            let same_block = found
                .ok()
                .flatten()
                .filter(|(state, _)| *state == delayed.state);
            if found.is_err() {
                digging.delayed = Some(delayed);
            } else if let (Some((state, rate)), Some(view)) = (same_block, view.as_ref()) {
                advance(&mut delayed.crack, rate, session, &mut changes);
                if delayed.crack.dig.progress() < 1.0 {
                    digging.delayed = Some(delayed);
                } else {
                    let hit = Hit {
                        pos,
                        state,
                        breaker: session,
                        dimension: dimension.0,
                    };
                    let new_states = break_block(
                        &definitions,
                        |pos| view.block(pos).ok(),
                        &hit,
                        &mut packets,
                        &mut changes,
                        &mut blocks,
                    );
                    broken_blocks.write_batch(harvest(
                        &definitions,
                        &hit,
                        &new_states,
                        |pos| view.block(pos).ok(),
                        player,
                    ));
                }
            }
        }
        !digging.is_idle()
    });
}
