use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Local, Query, Res};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_block_support::BlockSupport;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine::ability::{PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine::use_item_on_request::UseItemOnRequest;
use mcrs_minecraft_engine::{BlockBreakSet, eye_position, in_reach, reach};
use mcrs_minecraft_engine_item::{Selected, Slots, StackCommandsExt};
use mcrs_minecraft_engine_world::block_update::{BlockSetRequest, remove_block};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item_stack::{Count, Kind};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_player_slots::OFF_HAND;
use mcrs_minecraft_protocol_game::Hand;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockUpdate, ClientboundSystemChatPacket,
};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_text::{Color, IntoText, RawText, Text, TranslateArg};
use mcrs_minecraft_volume::{BlockVolume, Unavailable, Volume};
use mcrs_voxel::{BlockPos, BlockStateId};
use rustc_hash::FxHashMap;

use crate::{Click, Spot, placed_state, replaceable};

/// A block item used on a block face places its block (`block-placing.md`). It reads the
/// [`UseItemOnRequest`] messages, which the engine acknowledges.
#[derive(Clone)]
pub struct BlockPlacePlugin;

impl Plugin for BlockPlacePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            place_blocks
                .in_set(TickSet::ClientInput)
                .after(BlockBreakSet),
        );
    }
}

const OPERATOR_BLOCKS: [Block; 7] = [
    Block::CommandBlock,
    Block::ChainCommandBlock,
    Block::RepeatingCommandBlock,
    Block::StructureBlock,
    Block::Jigsaw,
    Block::TestBlock,
    Block::TestInstanceBlock,
];
const OPERATOR_LEVEL: u8 = 2;
const LOWEST_CURSOR: f32 = -0.5;
const HIGHEST_CURSOR: f32 = 1.5;

type PlacerItem<'a> = (
    &'a Owner,
    &'a PlayerGameMode,
    &'a InDimension,
    &'a Transform,
    Option<&'a PlayerOpLevel>,
    Option<&'a TeleportState>,
    Option<&'a Slots>,
    Option<&'a Selected>,
);

/// The checks of PLC-ORD-001 that this server has, in that order. Each request that reaches the
/// held item gets a refresh, after its placement when it places.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query, writer and resource is its own parameter"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the checks of PLC-ORD-001 run in their order over one request"
)]
fn place_blocks(
    mut requests: MessageReader<UseItemOnRequest>,
    placers: Query<PlacerItem>,
    bodies: Query<(&Transform, &PlayerGameMode), With<Player>>,
    stacks: Query<(&Kind, &Count)>,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    support: Option<Res<BlockSupport>>,
    items: Option<Res<Items>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut placed: Local<FxHashMap<BlockPos, BlockStateId>>,
    mut spent: Local<FxHashMap<Entity, u8>>,
    mut commands: Commands,
) {
    let (Some(definitions), Some(items)) = (definitions, items) else {
        requests.clear();
        return;
    };
    placed.clear();
    spent.clear();
    for request in requests.read() {
        if request.sequence.0 < 0 {
            continue;
        }
        let Ok((owner, mode, dimension, transform, op_level, teleport, slots, selected)) =
            placers.get(request.entity)
        else {
            continue;
        };
        let session = owner.0;
        let clicked = request.block_pos;
        let eye = eye_position(transform.translation);
        if !in_reach(eye, clicked, reach(mode.0)) || !cursor_in_range(request.cursor.to_array()) {
            continue;
        }
        let Some(view) = world.get(dimension.0) else {
            continue;
        };
        let bounds = view.bounds();
        let heights = (bounds.min.y, bounds.max.y);
        if !(heights.0..=heights.1).contains(&clicked.y) {
            let side = if clicked.y > heights.1 {
                Direction::Up
            } else {
                Direction::Down
            };
            packets.write_batch(height_message(session, side, heights));
            continue;
        }
        let void_air = definitions.default_state_of(Block::VoidAir.id());
        let block_at =
            |placed: &FxHashMap<BlockPos, BlockStateId>, pos: BlockPos| match placed.get(&pos) {
                Some(&state) => Some(state),
                None => match view.block(pos) {
                    Ok(state) => Some(state),
                    Err(Unavailable::Outside) => Some(void_air),
                    Err(_) => None,
                },
            };
        let adjacent = clicked + request.face.normal();
        let Some(clicked_state) = block_at(&placed, clicked) else {
            continue;
        };
        let adjacent_state = block_at(&placed, adjacent);
        let held = match request.hand {
            Hand::Main => selected.map(|selected| usize::from(selected.0)),
            Hand::Off => Some(usize::from(OFF_HAND)),
        }
        .and_then(|slot| slots?.get(slot))
        .and_then(|stack| {
            let (block, count) = block_item(&definitions, &items, &stacks, stack)?;
            (spent.get(&stack).copied().unwrap_or(0) < count).then_some((stack, block))
        });
        let teleporting = teleport.is_some_and(|teleport| teleport.pending().is_some());
        let placement = held.filter(|_| !teleporting).and_then(|(_, held)| {
            let placer = Placer {
                mode: mode.0,
                op_level: op_level.map_or(0, |level| level.clamped()),
                held,
                body: body_of(transform),
            };
            let click = Click {
                face: request.face,
                cursor_y: request.cursor.y,
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            };
            let others = bodies
                .iter()
                .filter(|(_, mode)| mode.0 != GameMode::Spectator)
                .map(|(transform, _)| body_of(transform));
            let spots = ((clicked, clicked_state), (adjacent, adjacent_state));
            let can_stay = |pos, state| {
                support.as_deref().is_none_or(|support| {
                    let read = |at| block_at(&placed, at).unwrap_or(void_air);
                    support.can_stay(state, pos, read)
                })
            };
            place(
                &definitions,
                &placer,
                &click,
                spots,
                can_stay,
                others,
                heights,
            )
        });
        if placement.is_none() && held.is_some() && at_limit(request.face, clicked.y, heights) {
            packets.write_batch(height_message(session, request.face, heights));
        }
        if let (Some((pos, new_state)), Some((stack, _))) = (placement, held) {
            if mode.0 != GameMode::Creative {
                commands.entity(stack).consume(1);
                *spent.entry(stack).or_default() += 1;
            }
            placed.insert(pos, new_state);
            blocks.write(BlockSetRequest {
                new_state,
                ..remove_block(dimension.0, pos)
            });
        }
        for pos in [clicked, adjacent] {
            if let Some(state) = block_at(&placed, pos) {
                packets.write(to(session, block_update(pos, state)));
            }
        }
    }
}

struct Placer {
    mode: GameMode,
    op_level: u8,
    held: Id<Block>,
    body: Aabb,
}

fn body_of(transform: &Transform) -> Aabb {
    Aabb::of_body(
        transform.translation,
        EntityTypeData::PLAYER.width,
        EntityTypeData::PLAYER.height,
    )
}

/// Steps 8 to 15 of PLC-ORD-001 for a placer that holds a block item: the position and the state
/// that the placement writes, or `None` for a refusal.
// chisle: the clicked block never acts and only players block a placement; adventure mode
// refuses each placement until the item reads `can_place_on`.
fn place(
    definitions: &BlockDefinitions,
    placer: &Placer,
    click: &Click,
    (clicked, adjacent): ((BlockPos, BlockStateId), (BlockPos, Option<BlockStateId>)),
    can_stay: impl FnOnce(BlockPos, BlockStateId) -> bool,
    mut others: impl Iterator<Item = Aabb>,
    (lowest, highest): (i32, i32),
) -> Option<(BlockPos, BlockStateId)> {
    if matches!(placer.mode, GameMode::Spectator | GameMode::Adventure) {
        return None;
    }
    let (target, replaced) = if replaceable(definitions, clicked.1, placer.held, Spot::Clicked) {
        clicked
    } else {
        (adjacent.0, adjacent.1?)
    };
    if !replaceable(definitions, replaced, placer.held, Spot::Adjacent) {
        return None;
    }
    let operator_block = OPERATOR_BLOCKS
        .iter()
        .any(|block| block.id() == placer.held);
    if operator_block && (placer.mode != GameMode::Creative || placer.op_level < OPERATOR_LEVEL) {
        return None;
    }
    let state = placed_state(definitions, placer.held, click, replaced)?;
    if !can_stay(target, state) {
        return None;
    }
    let origin = target.as_dvec3();
    let shape = definitions.shape(definitions.state(state).collision_shape);
    let overlaps = |body: &Aabb| {
        shape.iter().any(|bounds| {
            Aabb::new(
                origin + bounds.min.as_dvec3(),
                origin + bounds.max.as_dvec3(),
            )
            .intersects(body)
        })
    };
    if overlaps(&placer.body) || others.any(|body| overlaps(&body)) {
        return None;
    }
    (lowest..=highest)
        .contains(&target.y)
        .then_some((target, state))
}

fn at_limit(face: Direction, y: i32, (lowest, highest): (i32, i32)) -> bool {
    match face {
        Direction::Up => y >= highest,
        Direction::Down => y <= lowest,
        _ => false,
    }
}

fn cursor_in_range(cursor: [f32; 3]) -> bool {
    cursor
        .iter()
        .all(|value| (LOWEST_CURSOR..=HIGHEST_CURSOR).contains(value))
}

/// The block of the item of `stack` and the count of `stack`, when it is a block item.
fn block_item(
    definitions: &BlockDefinitions,
    items: &Items,
    stacks: &Query<(&Kind, &Count)>,
    stack: Entity,
) -> Option<(Id<Block>, u8)> {
    let (kind, count) = stacks.get(stack).ok()?;
    let state = items.get(Id::checked(kind.0, items.len())?)?.block_placer?;
    Some((definitions.block_index(state)?, count.get()))
}

/// The message of the upper height limit for `side` up, and of the lower one otherwise
/// (PLC-HGT-001).
fn height_message(
    session: PlayerSession,
    side: Direction,
    (lowest, highest): (i32, i32),
) -> Option<OutboundPlayerPacket> {
    let (key, limit) = if side == Direction::Up {
        ("build.tooHigh", highest)
    } else {
        ("build.tooLow", lowest)
    };
    let text: Text =
        Text::translate_args(key, vec![TranslateArg::Number(NbtTag::Int(limit))]).color(Color::RED);
    let content = RawText::from_text(&text).ok()?;
    Some(to(
        session,
        PacketPayload::SystemChat(ClientboundSystemChatPacket {
            content,
            overlay: true,
        }),
    ))
}

fn block_update(pos: BlockPos, state: BlockStateId) -> PacketPayload {
    PacketPayload::BlockUpdate(ClientboundBlockUpdate {
        block_pos: pos,
        block_state_id: BlockStateNumber(state.number()),
    })
}
