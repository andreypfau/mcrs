use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res, ResMut};
use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine::PlayerBody;
use mcrs_minecraft_engine::block_simulation::BlockMemory;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::dimension_change::Arrival;
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::{Bodies, EntityWrites};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity_rows::{EffectBuffer, Keyed, WriterKey, YzxPos};
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_physics::{Aabb, Axis};
use mcrs_minecraft_redstone::{Carried, HasRedstoneMemory, MovingPayload};
use mcrs_minecraft_simulation::BlockBehaviour;
use mcrs_voxel::BlockPos;

const ITEM_WIDTH: f32 = 0.25;
const ITEM_HEIGHT: f32 = 0.25;
const CLEARANCE: f64 = 0.01;
const LAUNCH: f64 = 1.0;
const TICKS_PER_MOVE: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mover {
    box_: Aabb,
    axis: Axis,
    sign: f64,
    slime: bool,
}

fn progress(payload: &MovingPayload, now: i64) -> f64 {
    let elapsed = now.saturating_sub(payload.started.0);
    (elapsed as f64 / TICKS_PER_MOVE).clamp(0.0, 1.0)
}

fn axis_of(direction: Direction) -> (Axis, f64) {
    let sign = if direction.is_positive() { 1.0 } else { -1.0 };
    let axis = match direction.axis() {
        mcrs_minecraft_core::Axis::X => Axis::X,
        mcrs_minecraft_core::Axis::Y => Axis::Y,
        mcrs_minecraft_core::Axis::Z => Axis::Z,
    };
    (axis, sign)
}

fn movers<'a>(
    blocks: &Blocks,
    moving: impl Iterator<Item = (&'a BlockPos, &'a MovingPayload)>,
    now: i64,
    out: &mut Vec<Mover>,
) {
    out.clear();
    let mut ordered: Vec<(&BlockPos, &MovingPayload)> = moving.collect();
    ordered.sort_by_key(|(pos, _)| (pos.x, pos.y, pos.z));
    for (pos, payload) in ordered {
        if payload.carried == Carried::Base {
            continue;
        }
        let (axis, sign) = axis_of(payload.facing);
        let left = 1.0 - progress(payload, now);
        let origin = pos.as_dvec3() - payload.facing.normal().as_dvec3() * left;
        let slime = blocks.block_index(payload.state) == Some(Block::SlimeBlock.id());
        let shape = blocks.shape(blocks.state(payload.state).collision_shape);
        for bounds in shape {
            out.push(Mover {
                box_: Aabb::new(
                    origin + bounds.min.as_dvec3(),
                    origin + bounds.max.as_dvec3(),
                ),
                axis,
                sign,
                slime,
            });
        }
    }
}

fn displacement(mover: &Mover, body: &Aabb) -> Option<DVec3> {
    if !mover.box_.intersects(body) {
        return None;
    }
    let distance = if mover.sign > 0.0 {
        mover.box_.max_of(mover.axis) - body.min_of(mover.axis) + CLEARANCE
    } else {
        mover.box_.min_of(mover.axis) - body.max_of(mover.axis) - CLEARANCE
    };
    let mut delta = DVec3::ZERO;
    match mover.axis {
        Axis::X => delta.x = distance,
        Axis::Y => delta.y = distance,
        Axis::Z => delta.z = distance,
    }
    Some(delta)
}

fn launch(mover: &Mover) -> DVec3 {
    let mut velocity = DVec3::ZERO;
    match mover.axis {
        Axis::X => velocity.x = mover.sign * LAUNCH,
        Axis::Y => velocity.y = mover.sign * LAUNCH,
        Axis::Z => velocity.z = mover.sign * LAUNCH,
    }
    velocity
}

type PlayerItem<'a> = (
    &'a Owner,
    &'a mut Transform,
    &'a mut TeleportState,
    PlayerBody,
);

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query, resource, writer and local is its own parameter"
)]
pub(crate) fn push_entities_with_moving_blocks<B>(
    memory: Res<BlockMemory<B>>,
    blocks: Res<Blocks>,
    time: Res<GameTime>,
    bodies: Res<Bodies>,
    mut writes: ResMut<EntityWrites>,
    mut items: Query<&mut MotionState, With<ItemEntity>>,
    mut players: Query<PlayerItem, With<Player>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut shapes: Local<Vec<Mover>>,
) where
    B: BlockBehaviour<WorldKinds> + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    let moving = &memory.0.redstone_ref().moving;
    if moving.is_empty() {
        return;
    }
    movers(&blocks, moving.iter(), time.0, &mut shapes);
    if shapes.is_empty() {
        return;
    }
    for mut state in &mut items {
        let mut body = Aabb::of_body(state.0.body.position, ITEM_WIDTH, ITEM_HEIGHT);
        for mover in shapes.iter() {
            let Some(delta) = displacement(mover, &body) else {
                continue;
            };
            if mover.slime {
                state.0.body.velocity = launch(mover);
            } else {
                state.0.body.position += delta;
                body = body.translate(delta);
            }
        }
    }
    let mut kicks = EffectBuffer::default();
    let mut sequence: u32 = 0;
    for (tile, bodies) in bodies.0.tiles() {
        for row in &bodies.rows {
            let mut body = row.bounds();
            for mover in shapes.iter() {
                let Some(delta) = displacement(mover, &body) else {
                    continue;
                };
                if !mover.slime {
                    body = body.translate(delta);
                }
                let effect = if mover.slime { launch(mover) } else { delta };
                let origin = BlockPos::new(
                    mover.box_.min.x.floor() as i32,
                    mover.box_.min.y.floor() as i32,
                    mover.box_.min.z.floor() as i32,
                );
                kicks.push(
                    tile,
                    Keyed {
                        target: row.key,
                        target_id: row.id,
                        writer: WriterKey::Block {
                            pos: YzxPos::from(origin),
                        },
                        sequence,
                        effect,
                    },
                );
                sequence = sequence.saturating_add(1);
            }
        }
    }
    writes.0.gather(&mut kicks);
    for (owner, mut transform, mut teleport, body) in &mut players {
        let dimensions = body.dimensions();
        let mut box_ = Aabb::of_body(transform.translation, dimensions.width, dimensions.height);
        let mut moved = DVec3::ZERO;
        for mover in shapes.iter() {
            if let Some(delta) = displacement(mover, &box_) {
                moved += delta;
                box_ = box_.translate(delta);
            }
        }
        if moved == DVec3::ZERO {
            continue;
        }
        transform.translation += moved;
        let arrival = Arrival::absolute(*transform);
        let teleport_id = teleport.sent(arrival.transform, *time);
        packets.write(to(
            owner.0,
            PacketPayload::PlayerPosition(arrival.position_packet(teleport_id)),
        ));
    }
}
