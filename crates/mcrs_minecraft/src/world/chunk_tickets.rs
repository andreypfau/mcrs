// use crate::world::chunk::{ChunkBundle, ChunkIndex, ChunkStatus};
// use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Plugin};
// use bevy_ecs::prelude::{DetectChangesMut, Resource};
// use bevy_ecs::system::{Commands, Query, Res, ResMut};
// use rustc_hash::FxHashMap;
// use std::cmp::Ordering;
// use std::collections::hash_map::Entry;
// use mcrs_engine::world::chunk::ChunkPos;
//
// pub struct ChunkTicketsPlugin;
//
// impl Plugin for ChunkTicketsPlugin {
//     fn build(&self, app: &mut App) {
//         app.insert_resource(ChunkTickets::default());
//         app.add_systems(FixedPreUpdate, spawn_chunks);
//         app.add_systems(FixedPostUpdate, tick_timeout);
//     }
// }
//
// pub fn tick_timeout(
//     mut chunk_tickets: ResMut<ChunkTickets>,
//     chunk_index: Res<ChunkIndex>,
//     mut chunk_statuses: Query<&mut ChunkStatus>,
// ) {
//     chunk_tickets.0.retain(|pos, tickets| {
//         tickets.retain_mut(|ticket| {
//             ticket.decrease_ticks_left();
//             !ticket.is_expired()
//         });
//         if tickets.is_empty() {
//             chunk_index.get(*pos)
//                 .and_then(|e| chunk_statuses.get_mut(e.chunk).ok())
//                 .map(|mut s| s.set_if_neq(ChunkStatus::Unloaded));
//             false
//         } else {
//             true
//         }
//     });
// }
//
// pub fn spawn_chunks(
//     chunk_tickets: Res<ChunkTickets>,
//     mut chunk_index: ResMut<ChunkIndex>,
//     mut commands: Commands,
// ) {
//     chunk_tickets.0.iter().for_each(|(pos, tickets)| {
//         if tickets.is_empty() {
//             return;
//         }
//         if chunk_index.contains(pos) {
//             return;
//         }
//         let chunk_entity = commands.spawn(ChunkBundle::new(*pos)).id();
//         chunk_index.insert(*pos, chunk_entity);
//         println!("Spawned chunk column at {:?}", pos);
//     })
// }
//
// #[derive(Debug, Clone, PartialEq, Eq, Ord)]
// pub struct Ticket {
//     pub kind: TicketKind,
//     pub level: u32,
//     pub ticks_left: i64,
// }
//
// impl Ticket {
//     pub fn new(kind: TicketKind, level: u32) -> Self {
//         Self {
//             kind,
//             level,
//             ticks_left: kind.timeout().unwrap_or(0) as i64,
//         }
//     }
//
//     pub fn decrease_ticks_left(&mut self) {
//         if self.kind.timeout().is_some() {
//             self.ticks_left -= 1;
//         }
//     }
//
//     pub fn is_expired(&self) -> bool {
//         if let Some(_) = self.kind.timeout() {
//             self.ticks_left < 0
//         } else {
//             false
//         }
//     }
//
//     /// Entity ticking occurs in load levels 31 and below.
//     /// This is the main load level type players will experience.
//     /// All game features are active here,
//     /// including entity processing (mobs wandering, spawning and despawning, etc.)
//     pub const LEVEL_ENTITY_TICKING: u32 = 31;
//
//     /// Ticking occurs at load level 32.
//     /// At this load level, all game features run as normal, except mobs are not being processed.
//     /// (They will not wander, despawn, etc.) Redstone componentry runs as normal in these chunks.
//     pub const LEVEL_BLOCK_TICKING: u32 = 32;
//
//     /// Border occurs at load level 33.
//     /// These chunks are "loaded" but typical game features do not work in them.
//     /// Redstone componentry does not function, command blocks don't tick, entities don't tick, etc.
//     /// However, mobs in border chunks do count towards the mob cap, for example.
//     pub const LEVEL_BORDER: u32 = 33;
//
//     /// Load levels 34 and up. These are not loaded chunks in any meaningful sense. However, world generation occurs on these chunks.
//     pub const LEVEL_INACCESSIBLE: u32 = 34;
// }
//
// impl PartialOrd for Ticket {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         let level_compare = self.level.cmp(&other.level);
//         if level_compare != Ordering::Equal {
//             return Some(level_compare);
//         }
//
//         Some(self.kind.cmp(&other.kind))
//     }
// }
//
// #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// pub enum TicketKind {
//     PlayerSpawn,
//     SpawnSearch,
//     Dragon,
//     PlayerLoading,
//     PlayerSimulation,
//     Forced,
//     Portal,
//     EnderPearl,
//     #[default]
//     Unknown,
// }
//
// impl TicketKind {
//     pub fn timeout(&self) -> Option<u64> {
//         match self {
//             TicketKind::PlayerSpawn => None,
//             TicketKind::SpawnSearch => Some(1),
//             TicketKind::Dragon => None,
//             TicketKind::PlayerLoading => None,
//             TicketKind::PlayerSimulation => None,
//             TicketKind::Forced => None,
//             TicketKind::Portal => Some(300),
//             TicketKind::EnderPearl => Some(40),
//             TicketKind::Unknown => Some(1),
//         }
//     }
// }
//
// #[derive(Resource, Debug, Default)]
// pub struct ChunkTickets(FxHashMap<ChunkPos, Vec<Ticket>>);
//
// impl ChunkTickets {
//     pub fn apply(&mut self, op: TicketOp) {
//         op.apply(self);
//     }
// }
//
// #[derive(Debug, Clone)]
// pub enum TicketOp {
//     Add {
//         chunk_pos: ChunkPos,
//         ticket_type: TicketKind,
//         level: u32,
//     },
//     Remove {
//         chunk_pos: ChunkPos,
//         ticket_type: TicketKind,
//         level: u32,
//     },
// }
//
// impl TicketOp {
//     fn apply(&self, tickets: &mut ChunkTickets) {
//         match self {
//             TicketOp::Add {
//                 chunk_pos,
//                 ticket_type,
//                 level,
//             } => {
//                 let ticket = Ticket::new(*ticket_type, *level);
//                 let vec = tickets.0.entry(*chunk_pos);
//                 match vec {
//                     Entry::Occupied(tickets) => {
//                         let v = tickets.into_mut();
//                         v.push(ticket);
//                         v.sort();
//                     }
//                     Entry::Vacant(a) => {
//                         a.insert(vec![ticket]);
//                     }
//                 }
//             }
//             TicketOp::Remove {
//                 chunk_pos,
//                 ticket_type,
//                 level,
//             } => {
//                 if let Some(list) = tickets.0.get_mut(chunk_pos) {
//                     if let Some(pos) = list
//                         .iter()
//                         .position(|t| t.kind == *ticket_type && t.level == *level)
//                     {
//                         list.remove(pos);
//                         // println!("Removed ticket: {chunk_pos:?}, current remaining: {:?}", list.len())
//                     }
//                 }
//             }
//         }
//     }
// }
