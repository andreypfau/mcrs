use crate::entity::physics::Transform;
use crate::world::chunk::ticket::{ChunkTicketsCommands, Ticket, TicketCommand, TicketKind};
use crate::world::chunk::{ChunkIndex, ChunkLoaded, ChunkPos};
use crate::world::dimension::{DimensionTypeConfig, InDimension};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::{
    Added, Changed, Component, ContainsEntity, Entity, EntityEvent, IntoScheduleConfigs,
    MessageWriter, Or, ParallelCommands, Query, With,
};
use bevy_ecs_macros::Message;
use std::collections::VecDeque;

const MAX_LOADS: usize = 256;

pub struct ChunkViewPlugin;

impl Plugin for ChunkViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerChunkLoadRequest>();
        app.add_message::<PlayerChunkUnloadRequest>();
        app.add_systems(
            FixedUpdate,
            (
                update_view,
                update_unload_queue,
                update_load_queue,
                update_loading_queue,
            )
                .chain(),
        );
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerViewDistance {
    pub distance: u8,
    pub vert_distance: u8,
}

impl Default for PlayerViewDistance {
    fn default() -> Self {
        Self {
            distance: 12,
            vert_distance: 8,
        }
    }
}

fn update_view(
    mut query: Query<
        (
            Entity,
            &mut PlayerChunkObserver,
            &Transform,
            &PlayerViewDistance,
            &InDimension,
        ),
        Or<(Changed<Transform>, Added<PlayerChunkObserver>)>,
    >,
    dimensions: Query<&DimensionTypeConfig>,
    commands: ParallelCommands,
) {
    query
        .par_iter_mut()
        .for_each(|(player, mut observer, transform, client_view_distance, in_dim)| {
            let observer = &mut *observer;
            // A dimension transition leaves last_last_chunk_tracking_view
            // referencing the previous dim's coordinate frame; diffing
            // against it would emit load/unload tickets for the new dim
            // using stale centre coordinates. Reset cached view state when
            // the player's InDimension changes so the next pass treats this
            // dim's first tick like a fresh spawn.
            let current_in_dim = in_dim.entity();
            if observer.last_in_dim != Some(current_in_dim) {
                observer.last_last_chunk_tracking_view = None;
                observer.unload_queue.clear();
                observer.load_queue.clear();
                observer.loading_queue.clear();
                observer.last_in_dim = Some(current_in_dim);
            }
            let chunk_pos = ChunkPos::from(transform.translation);
            let distance = client_view_distance.distance;
            let vert_distance = client_view_distance.vert_distance;
            let y_bounds = dimensions
                .get(in_dim.entity())
                .ok()
                .map(|cfg| {
                    let min_section_y = cfg.min_y >> 4;
                    let max_section_y = min_section_y + cfg.section_count as i32 - 1;
                    (min_section_y, max_section_y)
                })
                .unwrap_or((i32::MIN, i32::MAX));
            let new_view = ChunkTrackingView::with_y_bounds(
                chunk_pos,
                distance + 1,
                vert_distance + 1,
                y_bounds.0,
                y_bounds.1,
            );

            let Some(last_view) = observer.last_last_chunk_tracking_view else {
                let capacity = new_view.size();
                let mut load_queue = Vec::with_capacity(capacity);
                new_view.for_each(|pos| {
                    load_queue.push(pos);
                });
                load_queue.sort_unstable_by_key(|pos| pos.distance_squared(*chunk_pos));
                observer.load_queue.extend(load_queue);
                observer.last_last_chunk_tracking_view = Some(new_view);
                commands.command_scope(|mut cmd| {
                    cmd.trigger(ChunkTrackingViewUpdateEvent {
                        player,
                        old_view: None,
                        new_view,
                    });
                });
                return;
            };
            if new_view == last_view {
                return;
            }
            commands.command_scope(|mut cmd| {
                cmd.trigger(ChunkTrackingViewUpdateEvent {
                    player,
                    old_view: Some(last_view),
                    new_view,
                });
            });

            let mut load_queue = Vec::new();

            // println!("Updating chunk view from {:?} to {:?}", last_view, new_view);

            ChunkTrackingView::diff(&last_view, &new_view, |a| match a {
                ChunkViewAction::LoadChunk(pos) => {
                    load_queue.push(pos);
                }
                ChunkViewAction::UnloadChunk(pos) => {
                    observer.unload_queue.push_back(pos);
                }
            });

            load_queue.sort_unstable_by_key(|pos| pos.distance_squared(*chunk_pos));
            observer.load_queue.extend(load_queue);
            observer.last_last_chunk_tracking_view = Some(new_view);
        });
}

#[derive(Debug, Message)]
pub struct PlayerChunkUnloadRequest {
    pub player: Entity,
    pub chunk_pos: ChunkPos,
}

#[derive(Debug, Message)]
pub struct PlayerChunkLoadRequest {
    pub player: Entity,
    pub chunk_pos: ChunkPos,
    pub chunk: Entity,
}

fn update_unload_queue(
    mut query: Query<(Entity, &InDimension, &mut PlayerChunkObserver)>,
    mut dimensions: Query<&mut ChunkTicketsCommands>,
    mut unload_requests: MessageWriter<PlayerChunkUnloadRequest>,
) {
    query.iter_mut().for_each(|(player, dim, mut observer)| {
        let Ok(mut tickets) = dimensions.get_mut(dim.entity()) else {
            return;
        };
        let observer = &mut *observer;
        unload_requests.write_batch(observer.unload_queue.drain(..).map(|chunk_pos| {
            tickets.remove_ticket(chunk_pos, TicketKind::PlayerLoading);
            PlayerChunkUnloadRequest { player, chunk_pos }
        }));
    });
}

fn update_loading_queue(
    mut players: Query<(Entity, &mut PlayerChunkObserver, &InDimension)>,
    dims: Query<&ChunkIndex>,
    chunks: Query<Entity, With<ChunkLoaded>>,
    mut load_requests: MessageWriter<PlayerChunkLoadRequest>,
) {
    const MAX_SENDS: usize = 64 * 16;

    players.iter_mut().for_each(|(player, mut observer, dim)| {
        let Ok(chunk_index) = dims.get(**dim) else {
            return;
        };
        let observer = &mut *observer;
        let Some(last_view) = observer.last_last_chunk_tracking_view else {
            return;
        };
        let mut sends = 0;
        while sends < MAX_SENDS {
            let Some(chunk_pos) = observer.loading_queue.front().copied() else {
                return;
            };
            if !last_view.contains(&chunk_pos) {
                observer.loading_queue.pop_front();
                continue;
            }
            let Some(chunk) = chunk_index.get(chunk_pos) else {
                return;
            };
            if chunks.get(chunk).is_err() {
                return;
            };
            observer.loading_queue.pop_front();
            load_requests.write(PlayerChunkLoadRequest {
                player,
                chunk_pos,
                chunk,
            });
            sends += 1;
        }
    })
}

fn update_load_queue(
    mut players: Query<(&mut PlayerChunkObserver, &InDimension)>,
    mut dimensions: Query<&mut ChunkTicketsCommands>,
) {
    players.iter_mut().for_each(|(mut observer, _dim)| {
        let observer = &mut *observer;
        let Some(last_view) = observer.last_last_chunk_tracking_view else {
            return;
        };

        while observer.delayed_ticket_ops.len() < MAX_LOADS {
            let Some(pos) = observer.load_queue.pop_front() else {
                return;
            };
            if !last_view.contains(&pos) {
                continue;
            }
            observer.delayed_ticket_ops.push_back(TicketCommand::Add {
                chunk_pos: pos,
                ticket: Ticket::new(TicketKind::PlayerLoading),
            });
            observer.loading_queue.push_back(pos);
        }
    });
    players.iter_mut().for_each(|(mut observer, dim)| {
        if let Ok(mut chunks) = dimensions.get_mut(**dim) {
            observer
                .delayed_ticket_ops
                .drain(..)
                .for_each(|cmd| match cmd {
                    TicketCommand::Add { chunk_pos, ticket } => {
                        chunks.add_ticket(chunk_pos, ticket);
                    }
                    TicketCommand::Remove {
                        chunk_pos,
                        ticket_kind,
                    } => {
                        chunks.remove_ticket(chunk_pos, ticket_kind);
                    }
                });
        }
    });
}

#[deprecated(
    note = "Replaced by ChunkSubscriptionSet on the per-player AoI substrate. \
            Remaining consumers (worldgen tickets, column-view bookkeeping, \
            TNT view checks) are scheduled for migration in a follow-on cleanup."
)]
#[derive(Component, Debug, Default)]
pub struct PlayerChunkObserver {
    pub last_last_chunk_tracking_view: Option<ChunkTrackingView>,
    pub unload_queue: VecDeque<ChunkPos>,
    pub load_queue: VecDeque<ChunkPos>,
    pub loading_queue: VecDeque<ChunkPos>,
    pub delayed_ticket_ops: VecDeque<TicketCommand>,
    /// Last `InDimension` entity observed when `update_view` ran. A
    /// dimension transition leaves the cached `last_last_chunk_tracking_view`
    /// referencing the previous dim's coordinate frame, and the next diff
    /// would compute load/unload events against that stale centre. When
    /// this field disagrees with the current `InDimension`, the cached
    /// view and all in-flight queues reset.
    pub last_in_dim: Option<Entity>,
}

impl PlayerChunkObserver {
    pub fn can_view_chunk(&self, pos: &ChunkPos) -> bool {
        let Some(last_view) = self.last_last_chunk_tracking_view else {
            return false;
        };
        last_view.contains(pos)
    }
}

#[derive(EntityEvent)]
pub struct ChunkTrackingViewUpdateEvent {
    #[event_target]
    pub player: Entity,
    pub old_view: Option<ChunkTrackingView>,
    pub new_view: ChunkTrackingView,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct ChunkTrackingView {
    pub center: ChunkPos,
    pub distance: u8,
    pub vert_distance: u8,
    /// Inclusive lower bound on section_y; iteration and `contains` will
    /// reject positions below this. `i32::MIN` disables the floor.
    pub min_section_y: i32,
    /// Inclusive upper bound on section_y; `i32::MAX` disables the ceiling.
    pub max_section_y: i32,
}

impl Default for ChunkTrackingView {
    fn default() -> Self {
        Self {
            center: ChunkPos::new(0, 0, 0),
            distance: 12,
            vert_distance: 8,
            min_section_y: i32::MIN,
            max_section_y: i32::MAX,
        }
    }
}

pub enum ChunkViewAction {
    LoadChunk(ChunkPos),
    UnloadChunk(ChunkPos),
}

impl ChunkTrackingView {
    pub fn new(center: ChunkPos, distance: u8, vert_distance: u8) -> Self {
        Self::with_y_bounds(center, distance, vert_distance, i32::MIN, i32::MAX)
    }

    pub fn with_y_bounds(
        center: ChunkPos,
        distance: u8,
        vert_distance: u8,
        min_section_y: i32,
        max_section_y: i32,
    ) -> Self {
        Self {
            center,
            distance,
            vert_distance,
            min_section_y,
            max_section_y,
        }
    }

    fn min_x(&self) -> i32 {
        self.center.x - (self.distance as i32 + 1)
    }
    fn min_y(&self) -> i32 {
        let raw = self.center.y.saturating_sub(self.vert_distance as i32 + 1);
        raw.max(self.min_section_y)
    }
    fn min_z(&self) -> i32 {
        self.center.z - (self.distance as i32 + 1)
    }
    fn max_x(&self) -> i32 {
        self.center.x + (self.distance as i32 + 1)
    }
    fn max_y(&self) -> i32 {
        let raw = self.center.y.saturating_add(self.vert_distance as i32 + 1);
        raw.min(self.max_section_y)
    }
    fn max_z(&self) -> i32 {
        self.center.z + (self.distance as i32 + 1)
    }

    fn size(&self) -> usize {
        let y_extent = (self.max_y() - self.min_y() + 1).max(0) as usize;
        (self.distance as usize * 2 + 1) * (self.distance as usize * 2 + 1) * y_extent
    }

    fn intersects(&self, other: &ChunkTrackingView) -> bool {
        self.min_x() <= other.max_x()
            && self.max_x() >= other.min_x()
            && self.min_z() <= other.max_z()
            && self.max_z() >= other.min_z()
            && self.min_y() <= other.max_y()
            && self.max_y() >= other.min_y()
    }

    pub fn contains(&self, pos: &ChunkPos) -> bool {
        // Saturating ops keep the helper consistent with min_y / max_y,
        // which already use saturating arithmetic. Without this, an
        // extreme self.center.y would panic in debug builds here while
        // min_y / max_y silently clamp.
        let dy = pos.y.saturating_sub(self.center.y).unsigned_abs();
        let dx = pos.x.saturating_sub(self.center.x).unsigned_abs();
        let dz = pos.z.saturating_sub(self.center.z).unsigned_abs();
        pos.y >= self.min_section_y
            && pos.y <= self.max_section_y
            && dy <= self.vert_distance as u32
            && dx <= self.distance as u32
            && dz <= self.distance as u32
    }

    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(ChunkPos),
    {
        let d = self.distance as i32;
        let vd = self.vert_distance as i32;
        let cx = self.center.x;
        let cy = self.center.y;
        let cz = self.center.z;
        // Saturating ops mirror min_y / max_y so an extreme center.y or
        // an out-of-band distance cannot overflow the loop bounds.
        let y_lo = cy.saturating_sub(vd).max(self.min_section_y);
        let y_hi = cy.saturating_add(vd).min(self.max_section_y);
        let x_lo = cx.saturating_sub(d);
        let x_hi = cx.saturating_add(d);
        let z_lo = cz.saturating_sub(d);
        let z_hi = cz.saturating_add(d);
        for y in y_lo..=y_hi {
            for x in x_lo..=x_hi {
                for z in z_lo..=z_hi {
                    f(ChunkPos::new(x, y, z));
                }
            }
        }
    }

    pub fn diff<L>(old: &ChunkTrackingView, new: &ChunkTrackingView, mut callback: L)
    where
        L: FnMut(ChunkViewAction),
    {
        if old == new {
            return;
        }
        if !old.intersects(new) {
            old.for_each(|pos| callback(ChunkViewAction::UnloadChunk(pos)));
            new.for_each(|pos| callback(ChunkViewAction::LoadChunk(pos)));
            return;
        }
        let min_y = old.min_y().min(new.min_y());
        let max_y = old.max_y().max(new.max_y());
        let min_x = old.min_x().min(new.min_x());
        let min_z = old.min_z().min(new.min_z());
        let max_x = old.max_x().max(new.max_x());
        let max_z = old.max_z().max(new.max_z());

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                for z in min_z..=max_z {
                    let pos = ChunkPos::new(x, y, z);
                    let old_contains = old.contains(&pos);
                    let new_contains = new.contains(&pos);
                    if old_contains != new_contains {
                        if new_contains {
                            callback(ChunkViewAction::LoadChunk(pos));
                        } else {
                            callback(ChunkViewAction::UnloadChunk(pos));
                        }
                    }
                }
            }
        }
    }
}
