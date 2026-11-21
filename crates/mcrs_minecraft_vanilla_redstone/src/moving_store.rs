use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::{Added, With};
use bevy_ecs::system::{Commands, Query, Res, ResMut};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine::block_simulation::{BlockMemory, BlockRules, BlockSimulation};
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::{Dimension, InDimension};
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, BlockEntityPos, SectionBlockEntities, spawn_block_entities,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::to_nbt_compound;
use mcrs_minecraft_redstone::{
    Carried, HasRedstone, HasRedstoneMemory, Kind, MOVE_TICKS, MovingPayload, RedstoneTable,
};
use mcrs_minecraft_resource::{ResourceLocationBuf, parse_parts};
use mcrs_minecraft_simulation::{BlockBehaviour, GameTick, UpdateRequest, neighbour};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::block_entities::entity_at;

fn state_compound(blocks: &Blocks, state: BlockStateId) -> Option<NbtCompound> {
    let (name, properties) = blocks.properties_of(state.number())?;
    let mut compound = NbtCompound::new();
    compound.put_string("Name", name.to_string());
    if !properties.is_empty() {
        let mut values = NbtCompound::new();
        for (property, value) in properties {
            values.put_string(&property, value);
        }
        compound.put_component("Properties", values);
    }
    Some(compound)
}

fn state_of(blocks: &Blocks, compound: &NbtCompound) -> Option<BlockStateId> {
    let (namespace, path) = parse_parts(compound.get_string("Name")?)?;
    let name = ResourceLocationBuf::new(namespace, path).ok()?;
    let properties: Vec<(&str, &str)> = compound
        .get_compound("Properties")
        .map(|values| {
            values
                .child_tags
                .iter()
                .filter_map(|(property, value)| match value {
                    NbtTag::String(value) => Some((property.as_str(), value.as_str())),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let number = blocks.state_id_of(&name, &properties)?;
    BlockStateId::checked(number, blocks.state_count())
}

fn direction_index(direction: Direction) -> i32 {
    i32::try_from(direction.id()).unwrap_or(0)
}

fn direction_of(index: i32) -> Option<Direction> {
    Direction::all()
        .into_iter()
        .find(|direction| direction_index(*direction) == index)
}

fn carried_kind(table: &RedstoneTable, state: BlockStateId) -> Carried {
    match table.kind(state) {
        Some(Kind::PistonHead) => Carried::Head,
        Some(Kind::Piston { .. }) => Carried::Base,
        _ => Carried::Block,
    }
}

fn is_moving(table: &RedstoneTable, state: BlockStateId) -> bool {
    table.kind(state) == Some(Kind::MovingPiston)
}

fn block_entity_at<'a>(
    index: &SectionIndex,
    sections: &Query<&SectionBlockEntities>,
    positions: &Query<&BlockEntityPos>,
    entities: &'a Query<&BlockEntity>,
    pos: BlockPos,
) -> Option<&'a GeneratedBlockEntity> {
    let entity = entity_at(index, sections, positions, pos)?;
    entities.get(entity).ok().map(|entity| &entity.0)
}

fn carried_entity<'a>(
    at: &impl Fn(BlockPos) -> Option<&'a GeneratedBlockEntity>,
    source: BlockPos,
) -> Option<NbtCompound> {
    let entity = at(source)?;
    if matches!(entity, GeneratedBlockEntity::Piston { .. }) {
        return None;
    }
    to_nbt_compound(entity).ok()
}

fn moving_record<'a>(
    blocks: &Blocks,
    table: &RedstoneTable,
    at: &impl Fn(BlockPos) -> Option<&'a GeneratedBlockEntity>,
    pos: BlockPos,
    payload: &MovingPayload,
) -> Option<GeneratedBlockEntity> {
    let block_state = state_compound(blocks, payload.state)?;
    let carries_entity =
        payload.carried == Carried::Block && table.info(payload.state).block_entity;
    let carried = neighbour(pos, payload.facing.opposite())
        .filter(|_| carries_entity)
        .and_then(|source| carried_entity(at, source));
    Some(GeneratedBlockEntity::Piston {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        block_state,
        facing: direction_index(payload.facing),
        progress: 0.0,
        extending: payload.is_extending(),
        source: payload.carried != Carried::Block,
        carried,
    })
}

fn released_entity<'a>(
    at: &impl Fn(BlockPos) -> Option<&'a GeneratedBlockEntity>,
    pos: BlockPos,
) -> Option<GeneratedBlockEntity> {
    let GeneratedBlockEntity::Piston {
        carried: Some(carried),
        ..
    } = at(pos)?
    else {
        return None;
    };
    GeneratedBlockEntity::from_template(carried, pos, None)
        .ok()
        .flatten()
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query, resource and command buffer is its own parameter"
)]
pub(crate) fn record_moving_blocks<B>(
    mut placed: MessageReader<BlockPlaced>,
    rules: Res<BlockRules<B>>,
    memory: Res<BlockMemory<B>>,
    blocks: Res<Blocks>,
    dims: Query<(Entity, &SectionIndex), With<Dimension>>,
    sections: Query<&SectionBlockEntities>,
    positions: Query<&BlockEntityPos>,
    entities: Query<&BlockEntity>,
    mut commands: Commands,
) where
    B: BlockBehaviour<WorldKinds> + HasRedstone + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    let Ok((dimension, index)) = dims.single() else {
        placed.clear();
        return;
    };
    let at = |pos| block_entity_at(index, &sections, &positions, &entities, pos);
    let table = rules.0.redstone().table();
    let moving = &memory.0.redstone_ref().moving;
    let mut spawned = Vec::new();
    for change in placed.read() {
        let pos = change.block_pos;
        if is_moving(table, change.new_state) {
            if let Some(payload) = moving.get(&pos)
                && let Some(record) = moving_record(&blocks, table, &at, pos, payload)
            {
                spawned.push(record);
            }
            continue;
        }
        if is_moving(table, change.old_state)
            && table.info(change.new_state).block_entity
            && let Some(released) = released_entity(&at, pos)
        {
            spawned.push(released);
        }
    }
    if spawned.is_empty() {
        return;
    }
    spawn_block_entities(&mut commands, InDimension(dimension), spawned);
}

pub(crate) fn restore_moving_blocks<B>(
    loaded: Query<(&BlockEntityPos, &BlockEntity), Added<BlockEntity>>,
    rules: Res<BlockRules<B>>,
    blocks: Res<Blocks>,
    time: Res<GameTime>,
    mut memory: ResMut<BlockMemory<B>>,
    mut simulation: ResMut<BlockSimulation>,
) where
    B: BlockBehaviour<WorldKinds> + HasRedstone + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    let table = rules.0.redstone().table();
    for (pos, entity) in &loaded {
        let GeneratedBlockEntity::Piston {
            block_state,
            facing,
            ..
        } = &entity.0
        else {
            continue;
        };
        let moving = &mut memory.0.redstone().moving;
        if moving.contains_key(&pos.0) {
            continue;
        }
        let (Some(state), Some(facing)) = (state_of(&blocks, block_state), direction_of(*facing))
        else {
            continue;
        };
        moving.insert(
            pos.0,
            MovingPayload {
                state,
                facing,
                carried: carried_kind(table, state),
                started: GameTick(time.0.saturating_sub(i64::from(MOVE_TICKS))),
            },
        );
        simulation.request_update(UpdateRequest::Single {
            target: pos.0,
            block: Block::MovingPiston.id(),
        });
    }
}
