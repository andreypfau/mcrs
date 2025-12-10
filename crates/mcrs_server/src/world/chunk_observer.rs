use crate::client_info::ClientViewDistance;
use crate::world::chunk::{
    BiomesChunk, ChunkBlockStates, ChunkBundle, ChunkIndex, ChunkPlugin, ChunkStatus,
};
use crate::world::chunk_tickets::{ChunkTickets, Ticket, TicketKind, TicketOp};
use crate::world::paletted_container::bit_width;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::error::{error, panic};
use bevy_ecs::prelude::*;
use mcrs_network::ServerSideConnection;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundForgetLevelChunk, ClientboundLevelChunkWithLight,
    ClientboundSetChunkCacheCenter,
};
use mcrs_protocol::{BlockStateId, ChunkColumnPos, ChunkData, ChunkPos, Encode, LightData};
use mcrs_protocol::{Position, VarInt, WritePacket};
use rustc_hash::{FxHashMap, FxHashSet};
use std::char::MAX;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use bevy_ecs::entity::EntityHashSet;

pub struct ChunkObserverPlugin;

impl Plugin for ChunkObserverPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ChunkPlugin);
        app.add_systems(
            FixedUpdate,
            (
                update_view,
                update_unload_queue,
                update_chunk_pos_queue,
                update_load_queue,
                update_loading_queue,
                update_send_queue,
            )
                .chain(),
        );
    }
}

fn update_view(mut query: Query<(&mut PlayerChunkObserver, &Position, &ClientViewDistance)>) {
    query
        .par_iter_mut()
        .for_each(|(mut observer, position, client_view_distance)| {
            let observer = &mut *observer;
            let chunk_pos = ChunkColumnPos::from(*position);
            let client_view_distance = (**client_view_distance).clamp(2, 32);
            let new_view = ChunkColumnTrackingView::new(chunk_pos, client_view_distance + 1);

            let Some(last_view) = observer.last_last_chunk_tracking_view else {
                let mut load_queue = Vec::new();
                new_view.for_each(|pos| {
                    load_queue.push(pos);
                });
                load_queue.sort_by_key(|pos| pos.manhattan_distance(chunk_pos));
                load_queue.iter().for_each(|pos| {
                    println!("Initial load of chunk column {:?}", pos);
                });
                observer.load_queue.extend(load_queue);
                observer.update_center_queue.push_back(new_view.center);
                observer.update_radius_queue.push_back(new_view.distance);
                observer.last_last_chunk_tracking_view = Some(new_view);
                return;
            };
            if new_view == last_view {
                return;
            }
            if new_view.center != last_view.center {
                observer.update_center_queue.push_back(new_view.center);
            }
            if new_view.distance != last_view.distance {
                observer.update_radius_queue.push_back(new_view.distance);
            }


            let mut load_queue = Vec::new();

            println!("Updating chunk view from {:?} to {:?}", last_view, new_view);

            ChunkColumnTrackingView::diff(&last_view, &new_view, |(a)| match a {
                ChunkColumnObserverAction::LoadChunkColumn(pos) => {
                    load_queue.push(pos);
                }
                ChunkColumnObserverAction::UnloadChunkColumn(pos) => {
                    observer.unload_queue.push_back(pos);
                }
            });

            load_queue.sort_by_key(|pos| pos.distance_squared(chunk_pos));
            observer.load_queue.extend(load_queue);
            observer.last_last_chunk_tracking_view = Some(new_view);
        });
}

fn update_unload_queue(
    mut query: Query<(&mut PlayerChunkObserver, &mut ServerSideConnection)>,
    mut chunk_tickets: ResMut<ChunkTickets>,
) {
    query.iter_mut().for_each(|(mut observer, mut con)| {
        let observer = &mut *observer;
        observer.unload_queue.retain(|pos| {
            // println!("Process unload chunk column at {:?}", pos);
            for i in 0..16 {
                chunk_tickets.apply(TicketOp::Remove {
                    chunk_pos: ChunkPos::new(pos.x, i, pos.z),
                    ticket_type: TicketKind::PlayerLoading,
                    level: Ticket::LEVEL_ENTITY_TICKING,
                });
            }
            // con.write_packet(&ClientboundForgetLevelChunk {
            //     z: pos.z,
            //     x: pos.x,
            // });
            println!("Sent forget chunk column at {:?}", pos);
            false
        })
    });
}

fn update_chunk_pos_queue(mut query: Query<(&mut PlayerChunkObserver, &mut ServerSideConnection)>) {
    query.par_iter_mut().for_each(|(mut observer, mut con)| {
        observer.update_center_queue.retain(|pos| {
            con.write_packet(&ClientboundSetChunkCacheCenter {
                x: pos.x.into(),
                z: pos.z.into(),
            });
            // println!("Updating chunk cache center to {:?}", pos);
            false
        });
        observer.update_radius_queue.retain(|radius| {
            con.write_packet(&ClientboundChunkCacheRadius {
                radius: VarInt(*radius as i32),
            });
            // println!("Updating chunk cache radius to {}", radius);
            false
        })
    });
}

fn update_loading_queue(
    mut query: Query<(&mut PlayerChunkObserver)>,
    chunk_index: Res<ChunkIndex>,
    chunk_status: Query<&ChunkStatus>,
) {
    const MAX_SENDS: usize = 64;

    query.par_iter_mut().for_each(|mut observer| {
        let observer = &mut *observer;
        let Some(last_view) = observer.last_last_chunk_tracking_view else {
            return;
        };
        let loading_queue = &mut observer.loading_queue;

        while observer.send_queue.len() < MAX_SENDS {
            let Some(pos) = loading_queue.front().copied() else {
                return;
            };
            println!("Process loading chunk column at {:?}", pos);
            if !last_view.contains(&pos) {
                println!("[loading] Chunk column at {:?} is not in view, skip", pos);
                loading_queue.pop_front();
                continue;
            }
            for i in 0..16 {
                let chunk_pos = ChunkPos::new(pos.x, i, pos.z);
                let Some(chunk_entity) = chunk_index.get(chunk_pos) else {
                    println!("[loading] Chunk column at {:?} is not loaded, chunk y: {:?}, skip", pos, i);
                    return;
                };
                let Some(chunk_status) = chunk_status.get(chunk_entity.chunk).ok().copied() else {
                    println!("[loading] Chunk column at {:?} is not ready, chunk y: {:?}, skip", pos, i);
                    return;
                };
                if chunk_status != ChunkStatus::Ready {
                    println!("[loading] Chunk column at {:?} is not ready, chunk y: {:?}, skip", pos, i);
                    return;
                }
            }
            println!("[loading] Chunk column at {:?} is ready to send", pos);
            loading_queue.pop_front();
            observer.send_queue.push_back(pos);
        }
    });
}

fn update_load_queue(mut query: Query<(&mut PlayerChunkObserver)>, mut tickets: ResMut<ChunkTickets>) {
    const MAX_LOADS: usize = 64;
    query.par_iter_mut().for_each(|(mut observer)| {
        let observer = &mut *observer;

        let Some(last_view) = observer.last_last_chunk_tracking_view else {
            return;
        };

        let delayed_ticket_ops = &mut observer.delayed_ticket_ops;
        let load_queue = &mut observer.load_queue;
        let loading_queue = &mut observer.loading_queue;

        if !load_queue.is_empty() {
            println!("start processing load queue: {} entries, current loading: {}", load_queue.len(), loading_queue.len());
        }
        while loading_queue.len() < MAX_LOADS {
            let Some(pos) = load_queue.pop_front() else {
                return;
            };
            if !last_view.contains(&pos) {
                continue;
            }
            for i in 0..16 {
                delayed_ticket_ops.push_back(TicketOp::Add {
                    chunk_pos: ChunkPos::new(pos.x, i, pos.z),
                    ticket_type: TicketKind::PlayerLoading,
                    level: Ticket::LEVEL_ENTITY_TICKING,
                });
            }
            // println!("Loading chunk column at {:?}", pos);
            loading_queue.push_back(pos);
        }
    });
    query.iter_mut().for_each(|(mut observer)| {
        observer.flush_delayed_ticket_ops(&mut tickets);
    });
}

fn update_send_queue(
    mut query: Query<(&mut PlayerChunkObserver, &mut ServerSideConnection)>,
    chunks: Query<(&ChunkBlockStates, &BiomesChunk)>,
    chunk_index: Res<ChunkIndex>,
) {
    const MAX_SENDS: usize = 64;
    query.par_iter_mut().for_each(|(mut observer, mut con)| {
        let observer = &mut *observer;
        let Some(view) = observer.last_last_chunk_tracking_view else {
            return;
        };

        let mut sent = 0;
        while sent < MAX_SENDS {
            let Some(pos) = observer.send_queue.front().copied() else {
                return;
            };
            // println!("Process send chunk column at {:?}", pos);
            if !view.contains(&pos) {
                // println!("Chunk column at {:?} is not in view, skip", pos);
                observer.send_queue.pop_front();
                continue;
            }
            let mut chunks_data = Vec::with_capacity(16);
            for y in 0..16 {
                let chunk_pos = ChunkPos::new(pos.x, y, pos.z);
                let Some(chunk_entity) = chunk_index.get(chunk_pos).map(|e| e.chunk) else {
                    println!("Chunk column at {:?} is not loaded, chunk y: {:?}, skip", pos, y);
                    return;
                };

                let Some((blocks, biomes)) = chunks.get(chunk_entity).ok() else {
                    println!("Chunk column at {:?} is not ready, chunk y: {:?}, skip", pos, y);
                    return;
                };
                chunks_data.push((chunk_pos, blocks, biomes));
            }

            let mut data: Vec<u8> = Vec::new();
            for (_, blocks, biomes) in &chunks_data {
                blocks
                    .count_non_air_blocks()
                    .encode(&mut data)
                    .expect("Failed to encode chunk block count");

                blocks.0.convert_network()
                    .encode(&mut data)
                    .expect("Failed to encode chunk block states");

                biomes.0.convert_network()
                    .encode(&mut data)
                    .expect("Failed to encode chunk biomes");
            }
            for ((chunk_pos,_,_)) in &chunks_data {
                observer.sent_chunks.insert(*chunk_pos);
            }
            
            let pkt = ClientboundLevelChunkWithLight {
                pos,
                chunk_data: ChunkData {
                    data: data.as_slice(),
                    ..Default::default()
                },
                light_data: LightData::default(),
            };
            con.write_packet(&pkt);
            observer.send_queue.pop_front();
            // println!("Sent chunk column at {:?}", pos);
            sent += 1;
        }
    });
}

#[derive(Component, Debug, Default)]
pub struct PlayerChunkObserver {
    pub last_last_chunk_tracking_view: Option<ChunkColumnTrackingView>,
    pub unload_queue: VecDeque<ChunkColumnPos>,
    pub load_queue: VecDeque<ChunkColumnPos>,
    pub loading_queue: VecDeque<ChunkColumnPos>,
    pub send_queue: VecDeque<ChunkColumnPos>,
    pub sent_chunks: FxHashSet<ChunkPos>,
    pub update_center_queue: VecDeque<ChunkColumnPos>,
    pub update_radius_queue: VecDeque<u8>,
    pub delayed_ticket_ops: VecDeque<TicketOp>,
}

impl PlayerChunkObserver {
    pub fn can_view_chunk(&self, pos: &ChunkColumnPos) -> bool {
        let Some(last_view) = self.last_last_chunk_tracking_view else {
            return false;
        };
        last_view.contains(pos)
    }
}

#[derive(Component, Debug, Default)]
pub struct VisibleEntities {
    pub entities: EntityHashSet,
}

impl PlayerChunkObserver {
    pub fn flush_delayed_ticket_ops(&mut self, tickets: &mut ChunkTickets) {
        while let Some(op) = self.delayed_ticket_ops.pop_front() {
            tickets.apply(op);
        }
    }
}

fn are_neighbours_loaded(
    column_stages: &FxHashMap<ChunkColumnPos, ColumnLoadStage>,
    pos: &ChunkColumnPos,
) -> bool {
    let stage = column_stages.get(pos).copied().unwrap_or_default();
    if stage != ColumnLoadStage::Generated && stage != ColumnLoadStage::Tick {
        return false;
    }
    true
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
enum ColumnLoadStage {
    #[default]
    None,
    Loading,
    Loaded,
    Generating,
    Generated,
    Tick,
}

struct ChunkQueue {
    center: ChunkPos,
    heap: BinaryHeap<(Reverse<i32>, ChunkPos)>,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct ChunkColumnTrackingView {
    pub center: ChunkColumnPos,
    pub distance: u8,
}

impl Default for ChunkColumnTrackingView {
    fn default() -> Self {
        Self {
            center: ChunkColumnPos::new(0, 0),
            distance: 12,
        }
    }
}

pub enum ChunkColumnObserverAction {
    LoadChunkColumn(ChunkColumnPos),
    UnloadChunkColumn(ChunkColumnPos),
}

impl ChunkColumnTrackingView {
    pub fn new(center: ChunkColumnPos, distance: u8) -> Self {
        Self { center, distance }
    }

    fn min_x(&self) -> i32 {
        self.center.x - (self.distance as i32 + 1)
    }
    fn min_z(&self) -> i32 {
        self.center.z - (self.distance as i32 + 1)
    }
    fn max_x(&self) -> i32 {
        self.center.x + (self.distance as i32 + 1)
    }
    fn max_z(&self) -> i32 {
        self.center.z + (self.distance as i32 + 1)
    }

    const fn size(&self) -> usize {
        (self.distance as usize * 2 + 1) * (self.distance as usize * 2 + 1)
    }

    fn intersects(&self, other: &ChunkColumnTrackingView) -> bool {
        self.min_x() <= other.max_x()
            && self.max_x() >= other.min_x()
            && self.min_z() <= other.max_z()
            && self.max_z() >= other.min_z()
    }

    pub fn contains(&self, pos: &ChunkColumnPos) -> bool {
        let dx = pos.x - self.center.x;
        let dz = pos.z - self.center.z;
        dx.abs() <= self.distance as i32 && dz.abs() <= self.distance as i32
    }

    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(ChunkColumnPos),
    {
        let min_x = self.min_x();
        let min_z = self.min_z();
        let max_x = self.max_x();
        let max_z = self.max_z();
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let pos = ChunkColumnPos::new(x, z);
                if self.contains(&pos) {
                    f(pos);
                }
            }
        }
    }

    pub fn diff<L>(old: &ChunkColumnTrackingView, new: &ChunkColumnTrackingView, mut callback: L)
    where
        L: FnMut(ChunkColumnObserverAction),
    {
        if old == new {
            return;
        }
        if !old.intersects(new) {
            old.for_each(|pos| callback(ChunkColumnObserverAction::UnloadChunkColumn(pos)));
            new.for_each(|pos| callback(ChunkColumnObserverAction::LoadChunkColumn(pos)));
            return;
        }
        let min_x = old.min_x().min(new.min_x());
        let min_z = old.min_z().min(new.min_z());
        let max_x = old.max_x().max(new.max_x());
        let max_z = old.max_z().max(new.max_z());

        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let pos = ChunkColumnPos::new(x, z);
                let old_contains = old.contains(&pos);
                let new_contains = new.contains(&pos);
                if old_contains != new_contains {
                    if new_contains {
                        callback(ChunkColumnObserverAction::LoadChunkColumn(pos));
                    } else {
                        callback(ChunkColumnObserverAction::UnloadChunkColumn(pos));
                    }
                }
            }
        }
    }
}
