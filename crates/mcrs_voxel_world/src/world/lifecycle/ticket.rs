//! Chunk tickets.

use crate::entity::physics::Transform;
use crate::entity::player::Player;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::markers::ChunkFresh;
use crate::world::lifecycle::markers::ChunkLoaded;
use crate::world::lifecycle::markers::ChunkUnloaded;
use crate::world::lifecycle::markers::ChunkUnloading;
use crate::world::lifecycle::trace::{self, ColumnStage};
use crate::world::storage::chunk::Chunk;
use crate::world::storage::chunk::ChunkBundle;
use crate::world::storage::chunk::ChunkIndex;
use bevy_app::{App, First, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_ecs::query::With;
use indexmap::IndexMap;
use mcrs_voxel_math::ChunkPos;
use rustc_hash::{FxBuildHasher, FxHashMap};

/// Symmetric with the spawn cap: a pipeline that admits sections faster than it retires them
/// leaves dead sections sitting on the positions live ones are waiting for.
const MAX_DESPAWNS_PER_TICK: usize = MAX_SPAWNS_PER_TICK;
/// A view's row is 27 columns of 24 sections, and a column whose sections straddle two ticks
/// is sent a tick late, so the cap holds several rows. The view sizes its own intake against
/// this: raising more columns a tick than this can spawn grows the queue without bound.
pub const MAX_SPAWNS_PER_TICK: usize = 4096;

pub(crate) struct TicketPlugin;

/// Turns the tick's new tickets into chunk entities. It runs in `FixedUpdate` so the tickets a
/// view raised this tick become entities this tick, and whatever queues them can follow.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkSpawnSet;

impl Plugin for TicketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(First, expire_fresh_chunks);
        app.add_systems(FixedUpdate, spawn_chunks.in_set(ChunkSpawnSet));
        app.add_systems(
            FixedUpdate,
            (
                unload_chunks,
                // Before the spawn, so a section it takes leaves the index in the same run
                // that could hand its position to a fresh one.
                despawn_chunks.before(ChunkSpawnSet),
                remove_tickets_from_chunks,
            ),
        );
    }
}

/// Sections land after `FixedUpdate` has run, so a section's `ChunkFresh` has to
/// survive the whole tick after the one it landed in for the once-per-tick readers
/// of `Added<ChunkLoaded>` to see it.
fn expire_fresh_chunks(fresh: Query<(Entity, Ref<ChunkFresh>)>, mut commands: Commands) {
    for (entity, marker) in fresh.iter() {
        if !marker.is_added() {
            commands.entity(entity).try_remove::<ChunkFresh>();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
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
        if self.kind.timeout().is_some() {
            self.ticks_left < 0
        } else {
            false
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TicketKind {
    PlayerSimulation,
    Forced,
    #[default]
    Unknown,
}

impl TicketKind {
    pub fn timeout(&self) -> Option<u64> {
        match self {
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
    /// Queued ticket adds. The order they were raised in says nothing about what the player
    /// needs now, so `spawn_chunks` picks the nearest rather than the oldest; the map is
    /// insertion-ordered only so a tick's spawns are reproducible.
    add_tickets: IndexMap<ChunkPos, Vec<Ticket>, FxBuildHasher>,
    remove_tickets: FxHashMap<ChunkPos, Vec<TicketKind>>,
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

    /// Tickets raised for a section that has not been spawned yet.
    pub fn queued_tickets(&self, chunk_pos: ChunkPos) -> &[Ticket] {
        self.add_tickets
            .get(&chunk_pos)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// A ticket still queued is cancelled where it waits; one already handed to a
    /// spawned chunk has to be taken off that chunk's holder instead, or the chunk
    /// keeps a ticket nobody holds and never unloads.
    pub fn remove_ticket(&mut self, chunk_pos: ChunkPos, ticket_kind: TicketKind) {
        if let Some(tickets) = self.add_tickets.get_mut(&chunk_pos)
            && let Some(i) = tickets.iter().position(|t| t.kind == ticket_kind)
        {
            tickets.swap_remove(i);
            // A request nobody holds any more has to leave the queue, not stay as an empty
            // one: `spawn_chunks` would honour it, and the section it raised would be
            // condemned the same tick for holding no ticket. The queue would then never
            // shrink and the churn would crowd out the sections somebody is waiting for.
            if tickets.is_empty() {
                self.add_tickets.swap_remove(&chunk_pos);
            }
            return;
        }
        self.remove_tickets
            .entry(chunk_pos)
            .or_default()
            .push(ticket_kind);
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
    mut dims: Query<(&mut ChunkIndex, &ChunkTicketsCommands)>,
    chunk_statuses: Query<(Entity, &ChunkPos, &InDimension), With<ChunkUnloaded>>,
) {
    let mut taken = 0usize;
    for (chunk, chunk_pos, dim) in chunk_statuses.iter() {
        if taken >= MAX_DESPAWNS_PER_TICK {
            break;
        }
        let Ok((mut chunk_index, tickets)) = dims.get_mut(**dim) else {
            continue;
        };
        // Somebody asked for this section again while it was on its way out. `spawn_chunks`
        // brings it back with the blocks it already holds, which is the whole point of
        // leaving it alone: taking it now would mean generating the same terrain twice.
        if !tickets.queued_tickets(*chunk_pos).is_empty() {
            continue;
        }
        chunk_index.remove(*chunk_pos);
        commands.entity(chunk).try_despawn();
        taken += 1;
    }
}

fn unload_chunks(
    mut commands: Commands,
    chunk_statuses: Query<(Entity, &ChunkPos, &InDimension), With<ChunkUnloading>>,
) {
    chunk_statuses.iter().for_each(|(chunk, _chunk_pos, _dim)| {
        commands
            .entity(chunk)
            .try_remove::<ChunkUnloading>()
            .try_insert(ChunkUnloaded);
    })
}

/// Squared distance to the nearest player, or zero when the dimension holds none: with nobody
/// to be near, every section is equally worth spawning.
fn nearest_player_distance_sq(pos: ChunkPos, centers: &[ChunkPos]) -> i32 {
    centers
        .iter()
        .map(|center| pos.distance_squared(**center))
        .min()
        .unwrap_or(0)
}

/// A chunk awaiting despawn must not be handed the ticket: `despawn_chunks`
/// takes it with the entity, and nothing re-raises a ticket a view already
/// believes it has placed, so the section never loads again. The ticket stays
/// queued instead and spawns a fresh entity once the old one is gone.
pub fn spawn_chunks(
    mut dims: Query<(Entity, &mut ChunkTicketsCommands, &mut ChunkIndex)>,
    mut commands: Commands,
    mut chunks: Query<(Entity, &mut ChunkTicketHolder), With<Chunk>>,
    condemned: Query<(), Or<(With<ChunkUnloading>, With<ChunkUnloaded>)>>,
    players: Query<(&Transform, &InDimension), With<Player>>,
) {
    for (dim, mut chunk_tickets, mut chunk_index) in dims.iter_mut() {
        if chunk_tickets.add_tickets.is_empty() {
            continue;
        }

        // The backlog outlives the walk that raised it, so what is spawned first is chosen
        // against where the players stand now. Ticketing a section the player has since flown
        // past ahead of the one under their feet is what leaves a hole underneath them.
        let centers: Vec<ChunkPos> = players
            .iter()
            .filter(|(_, in_dim)| in_dim.entity() == dim)
            .map(|(transform, _)| ChunkPos::from(transform.translation))
            .collect();

        let mut keys_to_process: Vec<ChunkPos> =
            chunk_tickets.add_tickets.keys().copied().collect();
        if MAX_SPAWNS_PER_TICK < keys_to_process.len() {
            keys_to_process.select_nth_unstable_by_key(MAX_SPAWNS_PER_TICK, |pos| {
                nearest_player_distance_sq(*pos, &centers)
            });
            keys_to_process.truncate(MAX_SPAWNS_PER_TICK);
        }
        keys_to_process.sort_unstable_by_key(|pos| nearest_player_distance_sq(*pos, &centers));

        for pos in keys_to_process {
            let Some(tickets) = chunk_tickets.add_tickets.swap_remove(&pos) else {
                continue;
            };

            let Some(chunk_entity) = chunk_index.get(pos) else {
                trace::mark(pos.into(), ColumnStage::Spawned);
                let chunk_entity = commands
                    .spawn((
                        ChunkBundle::new(InDimension(dim), pos),
                        ChunkTicketHolder(tickets),
                    ))
                    .id();
                chunk_index.insert(pos, chunk_entity);
                continue;
            };

            // A section on its way out still holds the blocks it was generated with, so a
            // ticket arriving before it goes takes it back rather than waiting for the
            // despawn and generating the same terrain again. `ChunkLoaded` and `ChunkFresh`
            // go back on together: everything downstream keys off that pair to count the
            // section into its column and hand it to the light engine.
            if condemned.contains(chunk_entity) {
                commands
                    .entity(chunk_entity)
                    .try_remove::<ChunkUnloading>()
                    .try_remove::<ChunkUnloaded>()
                    .try_insert((ChunkLoaded, ChunkFresh));
            }
            if let Ok((_, mut ticket_holder)) = chunks.get_mut(chunk_entity) {
                ticket_holder.add_all(tickets);
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
                                .try_remove::<ChunkLoaded>()
                                .try_insert(ChunkUnloading);
                        }
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancelled_request_leaves_the_queue_rather_than_emptying_in_place() {
        let mut tickets = ChunkTicketsCommands::default();
        let pos = ChunkPos::new(0, 0, 0);
        tickets.add_ticket(pos, Ticket::new(TicketKind::Forced));
        assert_eq!(tickets.queued_tickets(pos).len(), 1);

        tickets.remove_ticket(pos, TicketKind::Forced);

        assert!(
            tickets.add_tickets.is_empty(),
            "the request is gone, and nothing was left for the spawn to honour"
        );
    }

    /// The backlog can hold more than a tick's worth of spawns, and what the player needs is
    /// what is under them, not what they asked for first.
    #[test]
    fn a_backlog_spawns_the_sections_nearest_the_player_first() {
        let mut app = App::new();
        app.add_systems(FixedUpdate, spawn_chunks);
        let dim = app
            .world_mut()
            .spawn((ChunkTicketsCommands::default(), ChunkIndex::new()))
            .id();
        app.world_mut()
            .spawn((Player, Transform::default(), InDimension(dim)));

        let under_the_player = ChunkPos::new(0, 0, 0);
        let mut dim_entity = app.world_mut().entity_mut(dim);
        let mut tickets = dim_entity
            .get_mut::<ChunkTicketsCommands>()
            .expect("ticket commands");
        // Raised first and far away, so insertion order alone would fill the whole tick.
        for x in 0..=MAX_SPAWNS_PER_TICK as i32 {
            tickets.add_ticket(
                ChunkPos::new(1000 + x, 0, 0),
                Ticket::new(TicketKind::Forced),
            );
        }
        tickets.add_ticket(under_the_player, Ticket::new(TicketKind::Forced));

        app.world_mut().run_schedule(FixedUpdate);

        let index = app.world().get::<ChunkIndex>(dim).expect("chunk index");
        assert!(
            index.get(under_the_player).is_some(),
            "the section under the player spawns even though it was asked for last"
        );
        assert!(
            index
                .get(ChunkPos::new(1000 + MAX_SPAWNS_PER_TICK as i32, 0, 0))
                .is_none(),
            "the furthest section is the one left for the next tick"
        );
    }

    /// Flying back over ground you just left is the common case at a large render distance.
    /// The section still holds its blocks, so taking the ticket back is a great deal cheaper
    /// than despawning it and generating the same terrain again.
    #[test]
    fn a_ticket_arriving_before_the_despawn_takes_the_section_back() {
        let mut app = App::new();
        app.add_systems(
            FixedUpdate,
            (
                despawn_chunks.before(ChunkSpawnSet),
                spawn_chunks.in_set(ChunkSpawnSet),
            ),
        );
        let dim = app
            .world_mut()
            .spawn((ChunkTicketsCommands::default(), ChunkIndex::new()))
            .id();
        let pos = ChunkPos::new(0, 0, 0);
        let dying = app
            .world_mut()
            .spawn((
                ChunkBundle::new(InDimension(dim), pos),
                ChunkTicketHolder(Vec::new()),
                ChunkUnloaded,
            ))
            .id();
        let mut dim_entity = app.world_mut().entity_mut(dim);
        dim_entity
            .get_mut::<ChunkIndex>()
            .expect("chunk index")
            .insert(pos, dying);
        dim_entity
            .get_mut::<ChunkTicketsCommands>()
            .expect("ticket commands")
            .add_ticket(pos, Ticket::new(TicketKind::Forced));

        app.world_mut().run_schedule(FixedUpdate);

        assert!(
            app.world().get_entity(dying).is_ok(),
            "the section that was asked for again is not taken"
        );
        assert_eq!(
            app.world()
                .get::<ChunkIndex>(dim)
                .expect("chunk index")
                .get(pos),
            Some(dying),
            "and it keeps its place in the index rather than being replaced"
        );
        assert!(app.world().get::<ChunkLoaded>(dying).is_some());
        assert!(app.world().get::<ChunkFresh>(dying).is_some());
        assert!(app.world().get::<ChunkUnloaded>(dying).is_none());
        assert_eq!(
            app.world()
                .get::<ChunkTicketHolder>(dying)
                .expect("ticket holder")
                .0
                .len(),
            1,
            "the ticket that saved it is the one it now holds"
        );
    }

    /// A ticket raised after the section is gone must still land, or the view believes it
    /// asked for something nothing will ever deliver.
    #[test]
    fn a_ticket_outlives_the_chunk_it_arrived_too_late_for() {
        let mut app = App::new();
        app.add_systems(
            FixedUpdate,
            (
                despawn_chunks.before(ChunkSpawnSet),
                spawn_chunks.in_set(ChunkSpawnSet),
            ),
        );
        let dim = app
            .world_mut()
            .spawn((ChunkTicketsCommands::default(), ChunkIndex::new()))
            .id();
        let pos = ChunkPos::new(0, 0, 0);
        let dying = app
            .world_mut()
            .spawn((
                ChunkBundle::new(InDimension(dim), pos),
                ChunkTicketHolder(Vec::new()),
                ChunkUnloaded,
            ))
            .id();
        let mut dim_entity = app.world_mut().entity_mut(dim);
        dim_entity
            .get_mut::<ChunkIndex>()
            .expect("chunk index")
            .insert(pos, dying);

        // Nobody wants it, so it goes.
        app.world_mut().run_schedule(FixedUpdate);
        assert!(app.world().get_entity(dying).is_err());

        app.world_mut()
            .entity_mut(dim)
            .get_mut::<ChunkTicketsCommands>()
            .expect("ticket commands")
            .add_ticket(pos, Ticket::new(TicketKind::Forced));
        app.world_mut().run_schedule(FixedUpdate);
        let respawned = app
            .world()
            .get::<ChunkIndex>(dim)
            .expect("chunk index")
            .get(pos)
            .expect("the ticket spawns a fresh section once the old one is gone");
        assert_ne!(respawned, dying);
    }

    #[test]
    fn chunk_fresh_lasts_through_the_tick_after_the_one_it_landed_in() {
        let mut app = App::new();
        app.add_systems(First, expire_fresh_chunks);
        app.update();

        let section = app.world_mut().spawn(ChunkLoaded).id();
        assert!(app.world().get::<ChunkFresh>(section).is_some());

        app.update();
        assert!(app.world().get::<ChunkFresh>(section).is_some());

        app.update();
        assert!(app.world().get::<ChunkFresh>(section).is_none());
    }
}
