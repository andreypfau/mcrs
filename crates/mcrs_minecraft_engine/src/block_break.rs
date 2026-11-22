use crate::advance_digs::{advance_digs, any_digger};
use crate::block_action_ack::BlockActionAck;
use crate::break_session::{ActiveDig, BreakSession, Crack, DelayedBreak};
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::client_on_ground::ClientOnGround;
use crate::column_change::ColumnChange;
use crate::dig_rate::{dig_rate, harvests};
use crate::diggers::Diggers;
use crate::movement::process_movement;
use crate::network_id::player_network_id;
use crate::player_dimensions::PlayerBody;
use crate::use_item_on_request::{UseItemOnRequest, handle_use_item_on};
use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::{Local, Query, Res, ResMut};
use bevy_math::{DVec3, IVec3};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags, Blocks, FluidState};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_block_break::{Dig, Finish};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine_world::block_drops::BlockBroken;
use mcrs_minecraft_engine_world::block_update::{BlockSetRequest, remove_block};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_pose::Dimensions;
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

const BREAK_EVENT: i32 = 2001;
const EXTINGUISH_EVENT: i32 = 1009;
const HIGHEST_STAYING_LEVEL: i32 = 8;
pub(crate) const DESTRUCTION_NONE: i8 = -1;

#[derive(Message)]
pub struct BlockActionRequest {
    pub action: PlayerAction,
    pub block_pos: BlockPos,
    pub face: Direction,
    pub entity: Entity,
    pub sequence: VarInt,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockBreakSet;

impl Plugin for BlockBreakPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockActionRequest>();
        app.add_message::<UseItemOnRequest>();
        app.add_message::<BlockActionAck>();
        app.add_message::<BlockBroken>();
        app.add_observer(handle_player_action);
        app.add_observer(handle_use_item_on);
        app.init_resource::<Diggers>();
        if let Err(error) = app.try_register_required_components::<Player, BreakSession>() {
            tracing::error!("a player does not require its dig session: {error}");
        }
        app.add_systems(
            FixedUpdate,
            (
                break_blocks.after(process_movement),
                advance_digs.run_if(any_digger),
            )
                .chain()
                .in_set(TickSet::ClientInput)
                .in_set(BlockBreakSet),
        );
        app.add_systems(
            FixedPostUpdate,
            send_acks.after(VoxelUpdateSet::NetworkSync),
        );
    }
}

fn send_acks(
    mut requests: MessageReader<BlockActionRequest>,
    mut uses: MessageReader<UseItemOnRequest>,
    owners: Query<&Owner>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut acks: MessageWriter<BlockActionAck>,
    mut highest: Local<FxHashMap<PlayerSession, i32>>,
) {
    let sequences = (requests
        .read()
        .map(|request| (request.entity, request.sequence)))
    .chain(
        uses.read()
            .map(|request| (request.entity, request.sequence)),
    );
    for (entity, sequence) in sequences {
        let Ok(&Owner(session)) = owners.get(entity) else {
            continue;
        };
        let value = sequence.0;
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

#[must_use]
pub fn reach(attributes: Option<&AttributeInstances>) -> Option<f64> {
    let range = match attributes {
        Some(attributes) => attributes.value(Attribute::BlockInteractionRange)?,
        None => {
            AttributeInstances::new(mcrs_minecraft_entity::attribute_bases(EntityType::Player)?)
                .value(Attribute::BlockInteractionRange)?
        }
    };
    Some(range + 1.0)
}

pub fn in_reach(eye: DVec3, block: BlockPos, limit: f64) -> bool {
    let min = block.as_dvec3();
    let max = min + DVec3::ONE;
    let nearest = eye.clamp(min, max);

    eye.distance_squared(nearest) < limit * limit
}

#[must_use]
pub fn eye_position(player: DVec3, dimensions: Dimensions) -> DVec3 {
    player + DVec3::new(0.0, f64::from(dimensions.eye_height), 0.0)
}

pub(crate) fn observed(
    pos: BlockPos,
    payload: PacketPayload,
    except: Option<PlayerSession>,
) -> ColumnChange {
    ColumnChange {
        column: ColumnPos::from(pos),
        payload,
        except,
    }
}

pub(crate) fn destruction(breaker: PlayerSession, pos: BlockPos, progress: i8) -> ColumnChange {
    observed(
        pos,
        PacketPayload::BlockDestruction(ClientboundBlockDestruction {
            id: VarInt(player_network_id(breaker)),
            pos,
            progress,
        }),
        Some(breaker),
    )
}

fn remove_cracks(
    changes: &mut MessageWriter<ColumnChange>,
    breaker: PlayerSession,
    dig: Option<ActiveDig>,
    pos: BlockPos,
) {
    if let Some(dig) = dig.filter(|dig| dig.crack.pos != pos) {
        changes.write(destruction(breaker, dig.crack.pos, DESTRUCTION_NONE));
    }
    changes.write(destruction(breaker, pos, DESTRUCTION_NONE));
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

pub(crate) fn level_event(effect: i32, pos: BlockPos, effect_data: i32) -> PacketPayload {
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

pub(crate) struct Hit {
    pub(crate) pos: BlockPos,
    pub(crate) state: BlockStateId,
    pub(crate) breaker: PlayerSession,
    pub(crate) dimension: Entity,
}

pub(crate) fn break_block(
    definitions: &BlockDefinitions,
    block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
    hit: &Hit,
    packets: &mut MessageWriter<OutboundPlayerPacket>,
    changes: &mut MessageWriter<ColumnChange>,
    blocks: &mut MessageWriter<BlockSetRequest>,
) -> SmallVec<[(BlockPos, BlockStateId); 2]> {
    let mut new_states = SmallVec::new();
    let owner = definitions.block_index(hit.state);
    if owner == Some(Block::Fire.id()) || owner == Some(Block::SoulFire.id()) {
        changes.write(observed(
            hit.pos,
            level_event(EXTINGUISH_EVENT, hit.pos, 0),
            None,
        ));
        blocks.write(remove_block(hit.dimension, hit.pos));
        new_states.push((hit.pos, BlockStateId::AIR));
        return new_states;
    }
    let hit_part = (hit.pos, hit.state);
    let mut events: SmallVec<[(BlockPos, BlockStateId); 3]> = SmallVec::new();
    events.push(hit_part);
    let mut changed: SmallVec<[(BlockPos, BlockStateId); 2]> = SmallVec::new();
    changed.push(hit_part);

    if let Some((other, other_state)) = paired_part(block_at, definitions, hit.state, hit.pos) {
        events.push((other.pos, other_state));
        let at = usize::from(!other.hit_is_main);
        changed.insert(at, (other.pos, other_state));
        if other.hit_is_main {
            events.push(hit_part);
        }
        let main = if other.hit_is_main {
            hit_part
        } else {
            (other.pos, other_state)
        };
        packets.write(to(hit.breaker, break_event(main.0, main.1)));
    }

    for (pos, state) in events {
        changes.write(observed(pos, break_event(pos, state), Some(hit.breaker)));
    }
    for (pos, state) in changed {
        if let Some(new_state) = state_after_break(definitions, state) {
            new_states.push((pos, new_state));
            blocks.write(BlockSetRequest {
                new_state,
                ..remove_block(hit.dimension, pos)
            });
        }
    }
    new_states
}

pub(crate) fn harvest(
    definitions: &BlockDefinitions,
    hit: &Hit,
    new_states: &[(BlockPos, BlockStateId)],
    block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
    player: Entity,
) -> Option<BlockBroken> {
    let &(_, left) = new_states.iter().find(|(at, _)| *at == hit.pos)?;
    let other = new_states
        .iter()
        .find(|(at, _)| *at != hit.pos)
        .and_then(|&(at, _)| Some((at, block_at(at)?)));
    Some(BlockBroken {
        at: hit.pos,
        state: hit.state,
        left,
        other,
        harvests: harvests(definitions, hit.state),
        breaker: player,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "the checks of one request run in the order that the specification states"
)]
fn break_blocks(
    mut requests: MessageReader<BlockActionRequest>,
    mut players: Query<(
        &Owner,
        &PlayerGameMode,
        PlayerBody,
        &InDimension,
        &Transform,
        &ClientOnGround,
        &mut BreakSession,
    )>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut changes: MessageWriter<ColumnChange>,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut broken_blocks: MessageWriter<BlockBroken>,
    mut diggers: ResMut<Diggers>,
    mut broken: Local<FxHashMap<BlockPos, BlockStateId>>,
) {
    let Some(definitions) = definitions else {
        requests.clear();
        return;
    };
    broken.clear();
    for request in requests.read() {
        let Ok((&Owner(session), mode, body, dimension, transform, &on_ground, mut digging)) =
            players.get_mut(request.entity)
        else {
            continue;
        };
        let pos = request.block_pos;
        let eye = eye_position(transform.translation, body.dimensions());
        let limit = reach(body.attributes);
        if !limit.is_some_and(|limit| in_reach(eye, pos, limit)) {
            let aborts = request.action == PlayerAction::AbortDestroyBlock;
            if mode.0 == GameMode::Survival && aborts && digging.dig.is_some() {
                remove_cracks(&mut changes, session, digging.dig.take(), pos);
            }
            continue;
        }
        let Some(view) = world.get(dimension.0) else {
            continue;
        };
        let block_at = |pos: BlockPos| match broken.get(&pos) {
            Some(&state) => Ok(state),
            None => view.block(pos),
        };
        let void_air = definitions.default_state_of(Block::VoidAir.id());
        if pos.y > view.bounds().max.y {
            packets.write(to(session, block_state(pos, void_air)));
            continue;
        }
        match (mode.0, request.action) {
            (_, PlayerAction::StartDestroyBlock)
            | (GameMode::Survival, PlayerAction::StopDestroyBlock) => {}
            (GameMode::Creative | GameMode::Survival, PlayerAction::AbortDestroyBlock) => {
                remove_cracks(&mut changes, session, digging.dig.take(), pos);
                continue;
            }
            (GameMode::Survival, PlayerAction::ChangeDestroyDirection) => {
                if let Some(dig) = digging.dig.as_mut() {
                    dig.face = request.face;
                }
                continue;
            }
            _ => continue,
        }

        let (old_state, inside) = match block_at(pos) {
            Ok(state) => (state, true),
            Err(Unavailable::Outside) => (void_air, false),
            Err(_) => continue,
        };
        match mode.0 {
            GameMode::Creative => {}
            GameMode::Adventure | GameMode::Spectator => {
                packets.write(to(session, block_state(pos, old_state)));
                continue;
            }
            GameMode::Survival => {
                let air = definitions
                    .state(old_state)
                    .flags
                    .contains(BlockStateFlags::IS_AIR);
                let rate = dig_rate(&definitions, &view, old_state, eye, on_ground);
                if request.action == PlayerAction::StartDestroyBlock {
                    if air || rate < 1.0 {
                        if let Some(replaced) = digging.dig
                            && let Ok(state) = block_at(replaced.crack.pos)
                        {
                            packets.write(to(session, block_state(replaced.crack.pos, state)));
                        }
                        let dig = Dig::start(rate);
                        let sent_stage = dig.stage();
                        digging.dig = Some(ActiveDig {
                            crack: Crack {
                                pos,
                                dig,
                                sent_stage,
                            },
                            face: request.face,
                        });
                        changes.write(destruction(session, pos, sent_stage.cast_signed()));
                        if !diggers.0.contains(&request.entity) {
                            diggers.0.push(request.entity);
                        }
                        continue;
                    }
                } else {
                    let Some(dig) = digging.dig.filter(|dig| dig.crack.pos == pos && !air) else {
                        continue;
                    };
                    match dig.crack.dig.finish(rate) {
                        Finish::Break => {
                            digging.dig = None;
                            changes.write(destruction(session, pos, DESTRUCTION_NONE));
                        }
                        Finish::DelayedBreak if digging.delayed.is_none() => {
                            digging.dig = None;
                            digging.delayed = Some(DelayedBreak {
                                crack: dig.crack,
                                state: old_state,
                            });
                            continue;
                        }
                        Finish::DelayedBreak | Finish::Stay => continue,
                    }
                }
            }
        }
        if !inside {
            changes.write(observed(pos, break_event(pos, old_state), Some(session)));
            continue;
        }
        let hit = Hit {
            pos,
            state: old_state,
            breaker: session,
            dimension: dimension.0,
        };
        let new_states = break_block(
            &definitions,
            |pos| block_at(pos).ok(),
            &hit,
            &mut packets,
            &mut changes,
            &mut blocks,
        );
        if mode.0 == GameMode::Survival {
            broken_blocks.write_batch(harvest(
                &definitions,
                &hit,
                &new_states,
                |pos| block_at(pos).ok(),
                request.entity,
            ));
        }
        broken.extend(new_states);
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
            face: p.direction,
            sequence: p.sequence,
            entity: on.entity,
        });
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_pose::Pose;

    use super::*;
    use crate::player_dimensions::player_dimensions;

    const CREATIVE_LIMIT: f64 = 6.0;

    struct ReachCase {
        name: &'static str,
        block: BlockPos,
        eye: DVec3,
        in_reach: bool,
    }

    #[test]
    fn brk_rch_001_the_reach_is_measured_to_the_nearest_point_of_the_cube() {
        let origin = BlockPos::new(0, 0, 0);
        let east = BlockPos::new(10, 0, 0);
        let cases = [
            ReachCase {
                name: "a gap of 5.99 to the side",
                block: east,
                eye: DVec3::new(10.0 - 5.99, 0.5, 0.5),
                in_reach: true,
            },
            ReachCase {
                name: "a gap of 6 to the side",
                block: east,
                eye: DVec3::new(10.0 - 6.0, 0.5, 0.5),
                in_reach: false,
            },
            ReachCase {
                name: "a gap of 5.95 above the top",
                block: origin,
                eye: DVec3::new(0.5, 1.0 + 5.95, 0.5),
                in_reach: true,
            },
            ReachCase {
                name: "a gap of 6.05 above the top",
                block: origin,
                eye: DVec3::new(0.5, 1.0 + 6.05, 0.5),
                in_reach: false,
            },
            ReachCase {
                name: "a diagonal of 4 to the corner",
                block: origin,
                eye: DVec3::new(-4.0, 0.5, -4.0),
                in_reach: true,
            },
            ReachCase {
                name: "a diagonal of 4.3 to the corner",
                block: origin,
                eye: DVec3::new(-4.3, 0.5, -4.3),
                in_reach: false,
            },
        ];

        for case in cases {
            assert_eq!(
                in_reach(case.eye, case.block, CREATIVE_LIMIT),
                case.in_reach,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn the_eye_of_a_crouching_player_is_at_its_f32_eye_height() {
        assert_eq!(
            eye_position(
                DVec3::new(1.0, 2.0, 3.0),
                player_dimensions(Pose::Crouching, None)
            ),
            DVec3::new(1.0, 2.0 + f64::from(1.27_f32), 3.0)
        );
    }
}
