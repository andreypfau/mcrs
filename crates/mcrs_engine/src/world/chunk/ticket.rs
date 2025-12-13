use crate::world::chunk::{ChunkBundle, ChunkIndex, ChunkPos, ChunkStatus};
use crate::world::dimension::InDimension;
use bevy::app::{FixedPostUpdate, FixedPreUpdate, FixedUpdate};
use bevy::log::Level;
use bevy::log::tracing::span;
use bevy::prelude::{
    Changed, Commands, Component, Deref, DerefMut, DetectChanges, DetectChangesMut, Entity,
    PostUpdate, PreUpdate, Query, Ref, Update,
};
use rustc_hash::FxHashMap;
use std::cmp::Ordering;
use tracing::info_span;

pub(crate) struct TicketPlugin;

impl bevy::prelude::Plugin for TicketPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(FixedPreUpdate, spawn_chunks);
        app.add_systems(FixedUpdate, unload_chunks);
        app.add_systems(FixedPostUpdate, (tick_timeout, despawn_chunks));
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

#[derive(Component, Debug, Default, DerefMut, Deref)]
pub struct ChunkTickets(FxHashMap<ChunkPos, Vec<Ticket>>);

impl ChunkTickets {
    pub fn add_ticket(&mut self, chunk_pos: ChunkPos, ticket: Ticket) {
        self.0.entry(chunk_pos).or_default().push(ticket);
        self.0.get_mut(&chunk_pos).map(|tickets| {
            tickets.sort_by(|a, b| b.cmp(a));
        });
    }

    pub fn remove_ticket(&mut self, chunk_pos: ChunkPos, ticket_kind: TicketKind) {
        if let Some(tickets) = self.0.get_mut(&chunk_pos) {
            // println!("Removing ticket {:?} from chunk {:?}", ticket_kind, chunk_pos);
            tickets
                .iter()
                .position(|t| t.kind == ticket_kind)
                .map(|i| tickets.remove(i));
            // println!("Chunk {:?} now has {} tickets", chunk_pos, tickets.len());
        }
    }
}

fn unload_chunks(mut chunk_statuses: Query<(&mut ChunkStatus), Changed<ChunkStatus>>) {
    chunk_statuses.iter_mut().for_each(|(mut status)| {
        if *status != ChunkStatus::Unloading {
            return;
        }
        status.set_if_neq(ChunkStatus::Unloaded);
    })
}

fn despawn_chunks(
    mut commands: Commands,
    mut dims: Query<(&mut ChunkIndex)>,
    chunk_statuses: Query<(Entity, &ChunkPos, &ChunkStatus, &InDimension), Changed<ChunkStatus>>,
) {
    chunk_statuses
        .iter()
        .for_each(|(chunk, chunk_pos, status, dim)| {
            if *status != ChunkStatus::Unloaded {
                return;
            }
            dims.get_mut(**dim).ok().map(|mut chunk_index| {
                chunk_index.remove(*chunk_pos);
            });
            commands.entity(chunk).despawn();
            // println!("Despawned chunk {:?}", chunk_pos);
        })
}

fn tick_timeout(
    mut dims: Query<(&mut ChunkTickets, &ChunkIndex)>,
    mut chunk_statuses: Query<&mut ChunkStatus>,
) {
    dims.iter_mut()
        .for_each(|(mut chunk_tickets, chunk_index)| {
            chunk_tickets.retain(|pos, tickets| {
                tickets.retain_mut(|t| {
                    t.decrease_ticks_left();
                    !t.is_expired()
                });
                if tickets.is_empty() {
                    chunk_index
                        .get(*pos)
                        .and_then(|e| chunk_statuses.get_mut(e).ok())
                        .map(|mut s| s.set_if_neq(ChunkStatus::Unloading));
                    false
                } else {
                    true
                }
            })
        })
}

fn spawn_chunks(
    mut dims: Query<(Entity, Ref<ChunkTickets>, &mut ChunkIndex), Changed<ChunkTickets>>,
    mut commands: Commands,
) {
    dims.iter_mut()
        .for_each(|(dim, chunk_tickets, mut chunk_index)| {
            let _span = info_span!("spawn_chunks iteration").entered();

            // Iterate only over new tickets by filtering out already spawned chunks
            let chunks_to_spawn: Vec<ChunkPos> = chunk_tickets
                .iter()
                .filter_map(|(pos, tickets)| {
                    if !tickets.is_empty() && !chunk_index.contains(*pos) {
                        Some(*pos)
                    } else {
                        None
                    }
                })
                .collect();

            drop(_span);

            // Spawn chunks in batch
            for pos in chunks_to_spawn {
                let chunk_entity = commands.spawn(ChunkBundle::new(InDimension(dim), pos)).id();
                chunk_index.insert(pos, chunk_entity);
            }
        })
}
