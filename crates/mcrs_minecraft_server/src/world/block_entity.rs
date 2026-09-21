use bevy_ecs::prelude::{Commands, Component};
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::BlockPos;
use bevy_ecs::system::Command;
use mcrs_minecraft_inventory::{Op, Slot, Transaction};
use mcrs_minecraft_item::{Items, SlotTable};
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::block_entity::BlockEntityPos;
use mcrs_minecraft_nbt::to_nbt_compound;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::chunk::ChunkDataBlockEntity;
use mcrs_minecraft_worldgen_feature_place::block_entity::BLOCK_ENTITY_TYPES;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use std::borrow::Cow;

/// A block entity's kind and state, in the one typed shape the save, the chunk
/// packet and the anvil reader share; the position is read off the entity's
/// [`BlockEntityPos`] and duplicated here only because the shape carries it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BlockEntity(pub GeneratedBlockEntity);

/// One entity per entry, under the dimension whose sections the reconcile step
/// resolves them against.
pub fn spawn_block_entities(
    commands: &mut Commands,
    dim: InDimension,
    entries: Vec<GeneratedBlockEntity>,
) {
    for entry in entries {
        let pos = entry.position();
        commands
            .spawn((
                BlockEntityPos(BlockPos::new(pos.x, pos.y, pos.z)),
                dim,
                BlockEntity(entry),
            ))
            .queue(fill_container);
    }
}

/// The saved `Items` list becomes the stack subtree and nothing else keeps
/// it; a slot outside the container is dropped as vanilla drops it.
fn fill_container(mut entity: EntityWorldMut) {
    let (block, items) = {
        let mut block_entity = entity.get_mut::<BlockEntity>().unwrap();
        match &mut block_entity.0 {
            GeneratedBlockEntity::Chest(data) => {
                ("minecraft:chest", std::mem::take(&mut data.items))
            }
            GeneratedBlockEntity::TrappedChest(data) => {
                ("minecraft:trapped_chest", std::mem::take(&mut data.items))
            }
            _ => return,
        }
    };
    let holder = entity.id();
    entity.world_scope(|world| {
        let Some(slot_count) = world
            .get_resource::<Blocks>()
            .and_then(|blocks| blocks.block(block))
            .and_then(|block| block.container_slots)
        else {
            return;
        };
        world.entity_mut(holder).insert(SlotTable::fixed(usize::from(slot_count)));
        if world.get_resource::<Items>().is_none() {
            return;
        }
        // One transaction per slot: a stack the corpus cannot spawn must not
        // take the rest of the container with it.
        for entry in items.into_iter().filter(|entry| entry.slot < slot_count) {
            Transaction(vec![Op::Spawn {
                value: entry.stack,
                to: Slot::new(holder, u16::from(entry.slot)),
            }])
            .apply(world);
        }
    });
}

/// The chunk packet's entry, carrying the same compound the save holds.
pub fn packet_entry(
    entry: &GeneratedBlockEntity,
) -> Result<ChunkDataBlockEntity<'static>, mcrs_minecraft_nbt::Error> {
    let pos = entry.position();
    let data = to_nbt_compound(entry)?;
    let id = data.get_string("id").expect("the enum is tagged by id");
    let kind = BLOCK_ENTITY_TYPES
        .iter()
        .position(|kind| *kind == id)
        .expect("every modelled block entity is a registered type") as i32;
    Ok(ChunkDataBlockEntity {
        packed_xz: (((pos.x & 15) << 4) | (pos.z & 15)) as i8,
        y: pos.y as i16,
        kind: VarInt(kind),
        data: Cow::Owned(data),
    })
}
