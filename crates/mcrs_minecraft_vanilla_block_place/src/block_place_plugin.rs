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
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::swing::SwingPlayer;
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine::use_item_on_request::UseItemOnRequest;
use mcrs_minecraft_engine::{BlockBreakSet, PlayerBody, eye_position, in_reach, reach};
use mcrs_minecraft_engine_item::{Selected, Slots, StackCommandsExt};
use mcrs_minecraft_engine_world::block_update::{BlockSetRequest, remove_block};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_inventory_player::OFF_HAND;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item_stack::{Count, InteractAnimation, Kind, SwingAnimation, SwingKind};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_pose::Dimensions;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockUpdate, ClientboundSystemChatPacket,
};
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_text::{Color, IntoText, RawText, Text, TranslateArg};
use mcrs_minecraft_volume::{BlockVolume, Unavailable, Volume};
use mcrs_voxel::{BlockPos, BlockStateId};
use rustc_hash::FxHashMap;

use crate::{Click, Spot, placed_state, replaceable};

#[derive(Clone)]
pub struct BlockPlacePlugin;

impl Plugin for BlockPlacePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SwingPlayer>().add_systems(
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
    PlayerBody,
    &'a InDimension,
    &'a Transform,
    Option<&'a PlayerOpLevel>,
    Option<&'a TeleportState>,
    Option<&'a Slots>,
    Option<&'a Selected>,
);

#[expect(
    clippy::too_many_lines,
    reason = "the checks of PLC-ORD-001 run in their order over one request"
)]
fn place_blocks(
    mut requests: MessageReader<UseItemOnRequest>,
    placers: Query<PlacerItem>,
    bodies: Query<(&Transform, &PlayerGameMode, PlayerBody), With<Player>>,
    stacks: Query<(&Kind, &Count, Option<&InteractAnimation>)>,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    support: Option<Res<BlockSupport>>,
    items: Option<Res<Items>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut swings: MessageWriter<SwingPlayer>,
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
        let Ok((owner, mode, body, dimension, transform, op_level, teleport, slots, selected)) =
            placers.get(request.entity)
        else {
            continue;
        };
        let session = owner.0;
        let clicked = request.block_pos;
        let dimensions = body.dimensions();
        let eye = eye_position(transform.translation, dimensions);
        let limit = reach(body.attributes);
        if !limit.is_some_and(|limit| in_reach(eye, clicked, limit))
            || !cursor_in_range(request.cursor.to_array())
        {
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
            let item = block_item(&definitions, &items, &stacks, stack)?;
            (spent.get(&stack).copied().unwrap_or(0) < item.count).then(|| HeldBlock {
                stack,
                block: item.block,
                swing: placement_swing(request.entity, request.hand, item.interact),
            })
        });
        let teleporting = teleport.is_some_and(|teleport| teleport.pending().is_some());
        let placement = held.filter(|_| !teleporting).and_then(|held_item| {
            let placer = Placer {
                mode: mode.0,
                op_level: op_level.map_or(0, |level| level.clamped()),
                held: held_item.block,
                body: body_of(transform, dimensions),
            };
            let click = Click {
                face: request.face,
                cursor_y: request.cursor.y,
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            };
            let others = bodies
                .iter()
                .filter(|(_, mode, ..)| mode.0 != GameMode::Spectator)
                .map(|(transform, _, body)| body_of(transform, body.dimensions()));
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
        if let (Some((pos, new_state)), Some(held_item)) = (placement, held) {
            if mode.0 != GameMode::Creative {
                commands.entity(held_item.stack).consume(1);
                *spent.entry(held_item.stack).or_default() += 1;
            }
            placed.insert(pos, new_state);
            blocks.write(BlockSetRequest {
                new_state,
                ..remove_block(dimension.0, pos)
            });
            swings.write(held_item.swing);
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

fn body_of(transform: &Transform, dimensions: Dimensions) -> Aabb {
    Aabb::of_body(transform.translation, dimensions.width, dimensions.height)
}

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

#[derive(Clone, Copy)]
struct HeldBlock {
    stack: Entity,
    block: Id<Block>,
    swing: SwingPlayer,
}

struct BlockItem {
    block: Id<Block>,
    count: u8,
    interact: Option<SwingAnimation>,
}

fn placement_swing(player: Entity, hand: Hand, animation: Option<SwingAnimation>) -> SwingPlayer {
    let animation = animation.unwrap_or_default();
    let kind = match animation.kind {
        SwingKind::None => SwingAnimationKind::None,
        SwingKind::Whack => SwingAnimationKind::Whack,
        SwingKind::Stab => SwingAnimationKind::Stab,
    };
    SwingPlayer::new(player, hand, kind, animation.duration.max(0))
}

fn block_item(
    definitions: &BlockDefinitions,
    items: &Items,
    stacks: &Query<(&Kind, &Count, Option<&InteractAnimation>)>,
    stack: Entity,
) -> Option<BlockItem> {
    let (kind, count, interact) = stacks.get(stack).ok()?;
    let state = items.get(Id::checked(kind.0, items.len())?)?.block_placer?;
    Some(BlockItem {
        block: definitions.block_index(state)?,
        count: count.get(),
        interact: interact.map(|animation| animation.0),
    })
}

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
