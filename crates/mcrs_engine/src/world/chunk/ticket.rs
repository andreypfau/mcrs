use crate::world::chunk::{
    Chunk, ChunkBundle, ChunkIndex, ChunkLoaded, ChunkPos, ChunkUnloaded, ChunkUnloading,
};
use crate::world::dimension::InDimension;
use bevy_app::{App, FixedPreUpdate, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_ecs::query::With;
use indexmap::IndexMap;
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::cmp::Ordering;

const MAX_DESPAWNS_PER_TICK: usize = 1024;
const MAX_SPAWNS_PER_TICK: usize = 1024;

pub(crate) struct TicketPlugin;

impl Plugin for TicketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedPreUpdate, spawn_chunks);
        app.add_systems(
            FixedUpdate,
            (unload_chunks, despawn_chunks, remove_tickets_from_chunks),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, Copy)]
pub struct Ticket {
    pub kind: TicketKind,
    pub ticks_left: i64,
}

impl Ticket {
    pub fn new(kind: TicketKind) -> Self {
        Self {
            kind,
            ticks_left: kind.timeout().unwrap_or(0) as i64,
        }
    }

    pub fn decrease_ticks_left(&mut self) {
        if self.kind.timeout().is_some() {
            self.ticks_left -= 1;
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(_) = self.kind.timeout() {
            self.ticks_left < 0
        } else {
            false
        }
    }
}

impl PartialOrd for Ticket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.kind.cmp(&other.kind))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TicketKind {
    PlayerLoading,
    PlayerSimulation,
    Forced,
    #[default]
    Unknown,
}

impl TicketKind {
    pub fn timeout(&self) -> Option<u64> {
        match self {
            TicketKind::PlayerLoading => None,
            TicketKind::PlayerSimulation => None,
            TicketKind::Forced => None,
            TicketKind::Unknown => Some(1),
        }
    }
}

#[derive(Component)]
pub struct TicketCommands {
    pub queue: Vec<Ticket>,
}

#[derive(Debug)]
pub enum TicketCommand {
    Add {
        chunk_pos: ChunkPos,
        ticket: Ticket,
    },
    Remove {
        chunk_pos: ChunkPos,
        ticket_kind: TicketKind,
    },
}

#[derive(Component, Debug, Default)]
pub struct ChunkTicketsCommands {
    /// Insertion-ordered map so chunks are spawned in the order they were requested
    /// (closest to player first, since the view system adds them in distance order).
    add_tickets: IndexMap<ChunkPos, Vec<Ticket>, FxBuildHasher>,
    remove_tickets: FxHashSet<(ChunkPos, Vec<TicketKind>)>,
}

#[derive(Component, Deref, DerefMut)]
pub struct ChunkTicketHolder(pub Vec<Ticket>);

impl ChunkTicketHolder {
    pub fn add(&mut self, ticket: Ticket) {
        self.0.push(ticket);
        self.0.sort_unstable_by(|a, b| b.cmp(a));
    }

    pub fn add_all(&mut self, tickets: Vec<Ticket>) {
        self.0.extend(tickets);
        self.0.sort_unstable_by(|a, b| b.cmp(a));
    }

    pub fn remove(&mut self, ticket_kind: TicketKind) {
        if let Some(i) = self.0.iter().position(|t| t.kind == ticket_kind) {
            self.0.swap_remove(i);
        }
    }
}

impl ChunkTicketsCommands {
    pub fn add_ticket(&mut self, chunk_pos: ChunkPos, ticket: Ticket) {
        self.add_tickets.entry(chunk_pos).or_default().push(ticket);
    }

    pub fn remove_ticket(&mut self, chunk_pos: ChunkPos, ticket_kind: TicketKind) {
        if let Some(tickets) = self.add_tickets.get_mut(&chunk_pos) {
            if let Some(i) = tickets.iter().position(|t| t.kind == ticket_kind) {
                tickets.swap_remove(i);
            }
        }
    }
}

// fn unload_chunks(
//     mut commands: Commands,
//     mut chunk_statuses: Query<(Entity, &ChunkStatus), Changed<ChunkStatus>>,
// ) {
//     chunk_statuses.iter_mut().for_each(|(e, status)| {
//         if *status != ChunkStatus::Unloading {
//             return;
//         }
//         commands.entity(e).insert(ChunkUnloaded);
//     })
// }

fn despawn_chunks(
    mut commands: Commands,
    mut dims: Query<(&mut ChunkIndex)>,
    chunk_statuses: Query<(Entity, &ChunkPos, &InDimension), With<ChunkUnloaded>>,
) {
    for (chunk, chunk_pos, dim) in chunk_statuses.iter().take(MAX_DESPAWNS_PER_TICK) {
        if let Ok(mut chunk_index) = dims.get_mut(**dim) {
            chunk_index.remove(*chunk_pos);
        }
        commands.entity(chunk).despawn();
    }
}

fn unload_chunks(
    mut commands: Commands,
    chunk_statuses: Query<(Entity, &ChunkPos, &InDimension), With<ChunkUnloading>>,
) {
    chunk_statuses.iter().for_each(|(chunk, chunk_pos, dim)| {
        commands
            .entity(chunk)
            .remove::<ChunkUnloading>()
            .insert(ChunkUnloaded);
    })
}

fn spawn_chunks(
    mut dims: Query<(Entity, &mut ChunkTicketsCommands, &mut ChunkIndex)>,
    mut commands: Commands,
    mut chunks: Query<(Entity, &mut ChunkTicketHolder), With<Chunk>>,
) {
    for (dim, mut chunk_tickets, mut chunk_index) in dims.iter_mut() {
        if chunk_tickets.add_tickets.is_empty() {
            continue;
        }

        let keys_to_process: Vec<_> = chunk_tickets
            .add_tickets
            .keys()
            .take(MAX_SPAWNS_PER_TICK)
            .copied()
            .collect();

        for pos in keys_to_process {
            let Some(tickets) = chunk_tickets.add_tickets.swap_remove(&pos) else {
                continue;
            };

            if !chunk_index.contains(pos) {
                let chunk_entity = commands
                    .spawn((
                        ChunkBundle::new(InDimension(dim), pos),
                        ChunkTicketHolder(tickets),
                    ))
                    .id();
                chunk_index.insert(pos, chunk_entity);
            } else {
                let Some(chunk_entity) = chunk_index.get(pos) else {
                    continue;
                };
                if let Ok((_, mut ticket_holder)) = chunks.get_mut(chunk_entity) {
                    ticket_holder.add_all(tickets);
                }
            }
        }
    }
}

fn remove_tickets_from_chunks(
    mut dims: Query<(&mut ChunkTicketsCommands, &ChunkIndex)>,
    mut chunks: Query<(Entity, &mut ChunkTicketHolder), With<Chunk>>,
    mut commands: Commands,
) {
    dims.iter_mut()
        .for_each(|(mut chunk_tickets, chunk_index)| {
            chunk_tickets
                .remove_tickets
                .drain()
                .for_each(|(pos, ticket_kinds)| {
                    let Some(chunk_entity) = chunk_index.get(pos) else {
                        return;
                    };
                    if let Ok((_, mut ticket_holder)) = chunks.get_mut(chunk_entity) {
                        ticket_kinds.iter().for_each(|kind| {
                            ticket_holder.remove(*kind);
                        });
                        if ticket_holder.0.is_empty() {
                            commands
                                .entity(chunk_entity)
                                .remove::<ChunkLoaded>()
                                .insert(ChunkUnloading);
                        }
                    }
                });
        });
}
