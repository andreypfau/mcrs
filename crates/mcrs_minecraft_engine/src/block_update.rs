use std::borrow::Cow;

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Query, Res};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntityPos, SectionBlockEntities,
};
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::chunk::ChunkBlockUpdateEntry;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockUpdate, ClientboundSectionBlocksUpdate,
};
use mcrs_minecraft_volume::{BlockVolume, SectionBlocks};
use mcrs_voxel::{BlockPos, ColumnPos, LocalPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::bus::PacketPayload;
use crate::column_change::ColumnChange;
use crate::column_streams::ColumnStreams;

#[derive(Resource, Default)]
pub struct BroadcastMarks {
    sections: FxHashMap<SectionPos, Vec<BlockPos>>,
    order: Vec<SectionPos>,
}

impl BroadcastMarks {
    pub fn mark(&mut self, pos: BlockPos) {
        let section = SectionPos::from(pos);
        let marks = self.sections.entry(section).or_insert_with(|| {
            self.order.push(section);
            Vec::new()
        });
        if !marks.contains(&pos) {
            marks.push(pos);
        }
    }

    pub fn broadcast<V: BlockVolume>(
        &mut self,
        blocks: &V,
        streams: Option<&ColumnStreams>,
        changes: &mut MessageWriter<ColumnChange>,
    ) {
        for section in self.order.drain(..) {
            let Some(marks) = self.sections.remove(&section) else {
                continue;
            };
            let column = ColumnPos::from(section);
            if !streams.is_some_and(|streams| streams.is_subscribed(column)) {
                continue;
            }
            let Ok(states) = blocks.section(section) else {
                continue;
            };
            let state_at = |pos: BlockPos| states.get(LocalPos::from(pos)).number();
            let payload = if let [pos] = marks[..] {
                PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                    block_pos: pos,
                    block_state_id: BlockStateNumber(state_at(pos)),
                })
            } else {
                let entries: Vec<ChunkBlockUpdateEntry> = marks
                    .iter()
                    .map(|&pos| {
                        let local = LocalPos::from(pos);
                        ChunkBlockUpdateEntry::new()
                            .with_off_x(local.x())
                            .with_off_y(local.y())
                            .with_off_z(local.z())
                            .with_block_state(state_at(pos))
                    })
                    .collect();
                PacketPayload::SectionBlocksUpdate(ClientboundSectionBlocksUpdate {
                    chunk_pos: section,
                    blocks: Cow::Owned(entries),
                })
            };
            changes.write(ColumnChange {
                column,
                payload,
                except: None,
            });
        }
    }
}

pub fn remove_replaced_block_entities(
    mut placed: MessageReader<BlockPlaced>,
    sections: Query<&SectionBlockEntities>,
    positions: Query<&BlockEntityPos>,
    blocks: Option<Res<Blocks>>,
    mut commands: Commands,
) {
    let Some(blocks) = blocks else {
        placed.clear();
        return;
    };
    for placed in placed.read() {
        if blocks.block_index(placed.old_state) == blocks.block_index(placed.new_state) {
            continue;
        }
        let Ok(held) = sections.get(placed.chunk) else {
            continue;
        };
        for &entity in held.iter() {
            if positions
                .get(entity)
                .is_ok_and(|at| at.0 == placed.block_pos)
            {
                commands.entity(entity).despawn();
            }
        }
    }
}
