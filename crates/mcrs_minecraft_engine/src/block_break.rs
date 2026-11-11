use crate::ability::PlayerGameMode;
use crate::block_action_ack::BlockActionAck;
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::column_change::ColumnChange;
use crate::movement::process_movement;
use crate::network_id::player_network_id;
use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Local, Query, Res};
use bevy_math::{DVec3, IVec3};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks, FluidState};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine_world::block_update::{BlockSetRequest, remove_block};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockDestruction, ClientboundBlockUpdate, ClientboundLevelEvent,
};
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_volume::{BlockVolume, Unavailable, Volume};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

pub struct BlockBreakPlugin;

const STANDING_EYE_HEIGHT: f64 = 1.62;
const REACH: f64 = 6.0;
const BREAK_EVENT: i32 = 2001;
const EXTINGUISH_EVENT: i32 = 1009;
const HIGHEST_STAYING_LEVEL: i32 = 8;
// 255 on the wire: the byte reads as -1 in this signed field.
const DESTRUCTION_NONE: i8 = -1;

#[derive(Message)]
pub struct BlockActionRequest {
    pub action: PlayerAction,
    pub block_pos: BlockPos,
    pub entity: Entity,
    pub sequence: VarInt,
}

impl Plugin for BlockBreakPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockActionRequest>();
        app.add_message::<BlockActionAck>();
        app.add_observer(handle_player_action);
        app.add_systems(
            FixedUpdate,
            break_blocks
                .in_set(TickSet::ClientInput)
                .after(process_movement),
        );
        app.add_systems(
            FixedPostUpdate,
            send_acks.after(VoxelUpdateSet::NetworkSync),
        );
    }
}

fn send_acks(
    mut requests: MessageReader<BlockActionRequest>,
    owners: Query<&Owner>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut acks: MessageWriter<BlockActionAck>,
    mut highest: Local<FxHashMap<PlayerSession, i32>>,
) {
    for request in requests.read() {
        let Ok(&Owner(session)) = owners.get(request.entity) else {
            continue;
        };
        let value = request.sequence.0;
        if value < 0 {
            packets.write(to(
                session,
                PacketPayload::Disconnect(Text::translate("disconnect.packetError", vec![])),
            ));
            continue;
        }
        highest
            .entry(session)
            .and_modify(|current| *current = (*current).max(value))
            .or_insert(value);
    }
    acks.write_batch(
        highest
            .drain()
            .map(|(session, sequence)| BlockActionAck { session, sequence }),
    );
}

fn in_reach(eye: DVec3, block: BlockPos) -> bool {
    let min = block.as_dvec3();
    let max = min + DVec3::ONE;
    let nearest = eye.clamp(min, max);

    eye.distance_squared(nearest) < REACH * REACH
}

fn eye_position(player: DVec3) -> DVec3 {
    player + DVec3::new(0.0, STANDING_EYE_HEIGHT, 0.0)
}

/// An event at `pos` for the clients that hold its column, but `except`. The muxer picks them,
/// because the players of the other cells of the world are not in this one.
fn observed(pos: BlockPos, payload: PacketPayload, except: Option<PlayerSession>) -> ColumnChange {
    ColumnChange {
        column: ColumnPos::from(pos),
        payload,
        except,
    }
}

fn block_state(pos: BlockPos, state: BlockStateId) -> PacketPayload {
    PacketPayload::BlockUpdate(ClientboundBlockUpdate {
        block_pos: pos,
        block_state_id: BlockStateNumber(state.number()),
    })
}

struct OtherPart {
    pos: BlockPos,
    hit_is_main: bool,
}

fn other_part(blocks: &BlockDefinitions, state: BlockStateId, pos: BlockPos) -> Option<OtherPart> {
    let entry = blocks.owner(state)?;
    let text = |property: &str| match entry.value_of(state, property) {
        Some(PropertyValue::Str(value)) => Some(value.as_ref()),
        _ => None,
    };
    let facing = || {
        Direction::all()
            .into_iter()
            .find(|direction| Some(direction.name()) == text("facing"))
            .map(|direction| direction.normal())
    };
    let (pos, hit_is_main) = match (text("half"), text("part")) {
        (Some("upper"), _) => (pos - IVec3::Y, true),
        (Some("lower"), _) => (pos + IVec3::Y, false),
        (_, Some("foot")) => (pos + facing()?, true),
        (_, Some("head")) => (pos - facing()?, false),
        _ => return None,
    };
    Some(OtherPart { pos, hit_is_main })
}

fn paired_part(
    block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
    blocks: &BlockDefinitions,
    hit: BlockStateId,
    pos: BlockPos,
) -> Option<(OtherPart, BlockStateId)> {
    let other = other_part(blocks, hit, pos)?;
    let state = block_at(other.pos)?;
    let answers = other_part(blocks, state, other.pos)?.pos == pos;
    let same = blocks.block_index(state) == blocks.block_index(hit);
    (answers && same).then_some((other, state))
}

fn level_event(effect: i32, pos: BlockPos, effect_data: i32) -> PacketPayload {
    PacketPayload::LevelEvent(ClientboundLevelEvent {
        effect,
        pos,
        effect_data,
        global: false,
    })
}

fn break_event(pos: BlockPos, state: BlockStateId) -> PacketPayload {
    level_event(BREAK_EVENT, pos, state.number() as i32)
}

fn state_after_break(blocks: &BlockDefinitions, old: BlockStateId) -> Option<BlockStateId> {
    let owner = blocks.block_index(old);
    if owner == Some(Block::Water.id()) || owner == Some(Block::Lava.id()) {
        let entry = blocks.owner(old)?;
        return match entry.value_of(old, FluidState::LEVEL) {
            Some(PropertyValue::Int(level)) if *level > HIGHEST_STAYING_LEVEL => entry.with(
                old,
                FluidState::LEVEL,
                &PropertyValue::Int(HIGHEST_STAYING_LEVEL),
            ),
            _ => None,
        };
    }
    Some(match blocks.state(old).fluid {
        Some(fluid) if fluid.fluid == Fluid::Water.id() => {
            blocks.default_state_of(Block::Water.id())
        }
        _ => BlockStateId::AIR,
    })
}

/// Breaks the blocks of the start actions of the tick. A block in a column of another cell
/// changes one tick later, when its owner applies the write.
#[expect(
    clippy::too_many_lines,
    reason = "the checks of one request run in the order that the specification states"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query, writer and resource is its own parameter"
)]
fn break_blocks(
    mut requests: MessageReader<BlockActionRequest>,
    players: Query<(&Owner, &PlayerGameMode, &InDimension, &Transform)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut changes: MessageWriter<ColumnChange>,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut broken: Local<FxHashMap<BlockPos, BlockStateId>>,
) {
    let Some(definitions) = definitions else {
        requests.clear();
        return;
    };
    broken.clear();
    for request in requests.read() {
        let Ok((&Owner(session), mode, dimension, transform)) = players.get(request.entity) else {
            continue;
        };
        if !in_reach(eye_position(transform.translation), request.block_pos) {
            continue;
        }
        let Some(view) = world.get(dimension.0) else {
            continue;
        };
        // A block that an earlier request of the tick broke reads as its new state.
        let block_at = |pos: BlockPos| match broken.get(&pos) {
            Some(&state) => Ok(state),
            None => view.block(pos),
        };
        let void_air = definitions.default_state_of(Block::VoidAir.id());
        if request.block_pos.y > view.bounds().max.y {
            packets.write(to(session, block_state(request.block_pos, void_air)));
            continue;
        }
        if request.action != PlayerAction::StartDestroyBlock {
            if request.action == PlayerAction::AbortDestroyBlock && mode.0 == GameMode::Creative {
                changes.write(observed(
                    request.block_pos,
                    PacketPayload::BlockDestruction(ClientboundBlockDestruction {
                        id: VarInt(player_network_id(session)),
                        pos: request.block_pos,
                        progress: DESTRUCTION_NONE,
                    }),
                    Some(session),
                ));
            }
            continue;
        }

        let (old_state, inside) = match block_at(request.block_pos) {
            Ok(state) => (state, true),
            Err(Unavailable::Outside) => (void_air, false),
            Err(_) => continue,
        };
        match mode.0 {
            GameMode::Creative => {}
            GameMode::Adventure | GameMode::Spectator => {
                packets.write(to(session, block_state(request.block_pos, old_state)));
                continue;
            }
            GameMode::Survival => continue,
        }
        let owner = definitions.block_index(old_state);
        if owner == Some(Block::Fire.id()) || owner == Some(Block::SoulFire.id()) {
            changes.write(observed(
                request.block_pos,
                level_event(EXTINGUISH_EVENT, request.block_pos, 0),
                None,
            ));
            broken.insert(request.block_pos, BlockStateId::AIR);
            blocks.write(remove_block(dimension.0, request.block_pos));
            continue;
        }
        let hit = (request.block_pos, old_state);
        let mut events: SmallVec<[(BlockPos, BlockStateId); 3]> = SmallVec::new();
        events.push(hit);
        let mut changed: SmallVec<[(BlockPos, BlockStateId); 2]> = SmallVec::new();
        changed.push(hit);

        if let Some((other, other_state)) = paired_part(
            |pos| block_at(pos).ok(),
            &definitions,
            old_state,
            request.block_pos,
        ) {
            events.push((other.pos, other_state));
            changed.push((other.pos, other_state));
            if other.hit_is_main {
                events.push(hit);
            }
            let main = if other.hit_is_main {
                hit
            } else {
                (other.pos, other_state)
            };
            packets.write(to(session, break_event(main.0, main.1)));
        }

        for (pos, state) in events {
            changes.write(observed(pos, break_event(pos, state), Some(session)));
        }

        if !inside {
            continue;
        }
        for (pos, state) in changed {
            if let Some(new_state) = state_after_break(&definitions, state) {
                broken.insert(pos, new_state);
                blocks.write(BlockSetRequest {
                    new_state,
                    ..remove_block(dimension.0, pos)
                });
            }
        }
    }
}

fn handle_player_action(
    on: On<Received<ServerboundPlayerAction>>,
    mut writer: MessageWriter<BlockActionRequest>,
) {
    let Some(p) = on.packet() else {
        return;
    };
    if matches!(
        p.action,
        PlayerAction::StartDestroyBlock
            | PlayerAction::ChangeDestroyDirection
            | PlayerAction::AbortDestroyBlock
            | PlayerAction::StopDestroyBlock
    ) {
        writer.write(BlockActionRequest {
            action: p.action,
            block_pos: p.pos,
            sequence: p.sequence,
            entity: on.entity,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brk_rch_001_a_distance_of_six_is_out_of_reach() {
        let block = BlockPos::new(10, 0, 0);
        let eye = |gap: f64| DVec3::new(10.0 - gap, 0.5, 0.5);

        assert!(in_reach(eye(5.99), block));
        assert!(!in_reach(eye(6.0), block));
    }

    #[test]
    fn brk_rch_001_from_above_the_distance_is_to_the_top_of_the_cube() {
        let block = BlockPos::new(0, 0, 0);
        let eye = |gap: f64| DVec3::new(0.5, 1.0 + gap, 0.5);

        assert!(in_reach(eye(5.95), block));
        assert!(!in_reach(eye(6.05), block));
    }

    #[test]
    fn brk_rch_001_on_a_diagonal_the_distance_is_to_the_corner_of_the_cube() {
        let block = BlockPos::new(0, 0, 0);

        assert!(in_reach(DVec3::new(-4.0, 0.5, -4.0), block));
        assert!(!in_reach(DVec3::new(-4.3, 0.5, -4.3), block));
    }

    #[test]
    fn a_standing_eye_is_above_the_position() {
        assert_eq!(
            eye_position(DVec3::new(1.0, 2.0, 3.0)),
            DVec3::new(1.0, 2.0 + STANDING_EYE_HEIGHT, 3.0)
        );
    }
}
