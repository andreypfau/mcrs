use crate::entity::physics::Transform;
use crate::world::dimension::{DimensionTypeConfig, InDimension};
use crate::world::lifecycle::ticket::MAX_SPAWNS_PER_TICK;
use crate::world::lifecycle::trace::{self, ColumnStage};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::{
    Added, Changed, Component, ContainsEntity, Entity, EntityEvent, IntoScheduleConfigs,
    MessageWriter, Or, ParallelCommands, Query,
};
use bevy_ecs::schedule::SystemSet;
use bevy_ecs_macros::Message;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_math::{ChunkPos, ColumnPos};
use rustc_hash::FxHashSet;
use std::collections::VecDeque;

pub struct ChunkViewPlugin;

/// The view diff and the load and unload requests it raises, so that what consumes them can
/// run in the same tick.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkViewSet;

impl Plugin for ChunkViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerChunkLoadRequest>();
        app.add_message::<PlayerChunkUnloadRequest>();
        app.add_systems(
            FixedUpdate,
            (update_view, update_unload_queue, update_load_queue)
                .chain()
                .in_set(ChunkViewSet),
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
    query.par_iter_mut().for_each(
        |(player, mut observer, transform, client_view_distance, in_dim)| {
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
                observer.last_in_dim = Some(current_in_dim);
            }
            let chunk_pos = ChunkPos::from(transform.translation);
            let distance = client_view_distance.distance;
            let vert_distance = client_view_distance.vert_distance;
            let y_bounds = dimensions
                .get(in_dim.entity())
                .ok()
                .map(|cfg| {
                    let min_section_y = cfg.min_y >> BLOCKS::BITS;
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
                observer.load_queue.reserve(new_view.column_count());
                new_view.each_column(|col| {
                    observer.load_queue.insert(col);
                });
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

            ChunkTrackingView::diff_columns(
                &last_view,
                &new_view,
                |col| {
                    observer.load_queue.insert(col);
                },
                |col| observer.unload_queue.push_back(col),
            );
            observer.last_last_chunk_tracking_view = Some(new_view);
        },
    );
}

#[derive(Debug, Message)]
pub struct PlayerChunkUnloadRequest {
    pub player: Entity,
    pub column_pos: ColumnPos,
}

#[derive(Debug, Message)]
pub struct PlayerChunkLoadRequest {
    pub player: Entity,
    pub column_pos: ColumnPos,
}

fn update_unload_queue(
    mut query: Query<(Entity, &mut PlayerChunkObserver)>,
    mut unload_requests: MessageWriter<PlayerChunkUnloadRequest>,
) {
    query.iter_mut().for_each(|(player, mut observer)| {
        unload_requests.write_batch(
            observer
                .unload_queue
                .drain(..)
                .map(|column_pos| PlayerChunkUnloadRequest { player, column_pos }),
        );
    });
}

/// Raises what the view wants with the send layer, nearest first.
///
/// It raises columns and holds no ticket of its own: the send layer needs every section of a
/// column and already forces them, so a second ticket over the vertical slice of the view
/// would only duplicate that — and duplicate it faster than sections can be spawned.
///
/// The budget is columns rather than sections for the same reason: raising a column costs a
/// whole column's worth of spawns, so counting sections here would let the view ask for more
/// than `spawn_chunks` can ever hand out and grow the queue without bound.
fn update_load_queue(
    mut players: Query<(Entity, &mut PlayerChunkObserver, &InDimension)>,
    dimensions: Query<&DimensionTypeConfig>,
    mut load_requests: MessageWriter<PlayerChunkLoadRequest>,
) {
    players.iter_mut().for_each(|(player, mut observer, dim)| {
        let observer = &mut *observer;
        let Some(last_view) = observer.last_last_chunk_tracking_view else {
            return;
        };
        observer
            .load_queue
            .retain(|col| last_view.contains_column(col.x, col.z));
        if observer.load_queue.is_empty() {
            return;
        }
        let sections_per_column = dimensions
            .get(dim.entity())
            .map(|config| config.section_count.max(1) as usize)
            .unwrap_or(1);
        let budget = (MAX_SPAWNS_PER_TICK / sections_per_column).max(1);

        let center = last_view.center;
        let mut nearest: Vec<ColumnPos> = observer.load_queue.iter().copied().collect();
        if budget < nearest.len() {
            nearest.select_nth_unstable_by_key(budget, |col| {
                col.distance_squared(ColumnPos::new(center.x, center.z))
            });
            nearest.truncate(budget);
        }
        nearest
            .sort_unstable_by_key(|col| col.distance_squared(ColumnPos::new(center.x, center.z)));

        for column_pos in nearest {
            observer.load_queue.remove(&column_pos);
            trace::mark(column_pos, ColumnStage::Ticketed);
            load_requests.write(PlayerChunkLoadRequest { player, column_pos });
        }
    });
}

/// `ChunkSubscriptionSet` on the per-player AoI substrate is the source of
/// truth for what a player observes. These queues are a second representation
/// of the same fact, still read by worldgen ticketing, column-view bookkeeping
/// and explosion view checks; until those read the subscription set instead, the two
/// must be kept in step.
#[derive(Component, Debug, Default)]
pub struct PlayerChunkObserver {
    pub last_last_chunk_tracking_view: Option<ChunkTrackingView>,
    pub unload_queue: VecDeque<ColumnPos>,
    /// Wanted by the view, not yet raised with the send layer. Unordered: what is raised next
    /// is the nearest to where the player is now, and a player crossing the view invalidates
    /// any order settled when a column was first revealed.
    pub load_queue: FxHashSet<ColumnPos>,
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

    fn intersects(&self, other: &ChunkTrackingView) -> bool {
        self.min_x() <= other.max_x()
            && self.max_x() >= other.min_x()
            && self.min_z() <= other.max_z()
            && self.max_z() >= other.min_z()
            && self.min_y() <= other.max_y()
            && self.max_y() >= other.min_y()
    }

    /// Whether any section of this column is still tracked. The unload queue
    /// is per section, so a column only truly leaves the view when its XZ does.
    pub fn contains_column(&self, x: i32, z: i32) -> bool {
        x.saturating_sub(self.center.x).unsigned_abs() <= self.distance as u32
            && z.saturating_sub(self.center.z).unsigned_abs() <= self.distance as u32
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

    /// Every column the view holds. The vertical extent is not part of it: a column is sent
    /// whole, so what the player is owed is decided in XZ alone.
    fn each_column(&self, mut f: impl FnMut(ColumnPos)) {
        let d = self.distance as i32;
        for x in self.center.x.saturating_sub(d)..=self.center.x.saturating_add(d) {
            for z in self.center.z.saturating_sub(d)..=self.center.z.saturating_add(d) {
                f(ColumnPos::new(x, z));
            }
        }
    }

    fn column_count(&self) -> usize {
        let span = self.distance as usize * 2 + 1;
        span * span
    }

    /// The columns the move added and dropped. A step that only changes the player's height
    /// moves no column either way, which is what keeps a vertical step from tearing down and
    /// re-sending the whole view.
    pub fn diff_columns(
        old: &ChunkTrackingView,
        new: &ChunkTrackingView,
        mut on_load: impl FnMut(ColumnPos),
        mut on_unload: impl FnMut(ColumnPos),
    ) {
        if old.center.x == new.center.x
            && old.center.z == new.center.z
            && old.distance == new.distance
        {
            return;
        }
        let min_x = old.min_x().min(new.min_x());
        let max_x = old.max_x().max(new.max_x());
        let min_z = old.min_z().min(new.min_z());
        let max_z = old.max_z().max(new.max_z());
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let was = old.contains_column(x, z);
                let is = new.contains_column(x, z);
                match (was, is) {
                    (false, true) => on_load(ColumnPos::new(x, z)),
                    (true, false) => on_unload(ColumnPos::new(x, z)),
                    _ => {}
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

#[cfg(test)]
mod contains_column_tests {
    use super::*;

    #[test]
    fn a_vertical_step_keeps_every_column_the_view_already_held() {
        let view = ChunkTrackingView::new(ChunkPos::new(0, 4, 0), 12, 8);
        let stepped = ChunkTrackingView::new(ChunkPos::new(0, 5, 0), 12, 8);

        // The section that drops out of the bottom of the vertical window.
        let evicted = ChunkPos::new(3, -4, 7);
        assert!(view.contains(&evicted));
        assert!(!stepped.contains(&evicted));

        // Its column is still tracked, so the column must not be torn down.
        assert!(stepped.contains_column(evicted.x, evicted.z));
    }

    #[test]
    fn a_column_outside_the_horizontal_reach_is_gone() {
        let view = ChunkTrackingView::new(ChunkPos::new(0, 4, 0), 12, 8);
        assert!(view.contains_column(12, -12));
        assert!(!view.contains_column(13, 0));
        assert!(!view.contains_column(0, -13));
    }
}

#[cfg(test)]
mod load_queue_tests {
    use super::*;
    use crate::world::dimension::DimensionTypeConfig;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    fn requests(world: &mut World) -> Vec<ColumnPos> {
        world
            .resource_mut::<Messages<PlayerChunkLoadRequest>>()
            .drain()
            .map(|req| req.column_pos)
            .collect()
    }

    fn one_player_looking_at_the_origin(world: &mut World, wants: &[ColumnPos]) -> Entity {
        world.init_resource::<Messages<PlayerChunkLoadRequest>>();
        let dim = world.spawn(DimensionTypeConfig::new(0, 16)).id();
        let observer = PlayerChunkObserver {
            last_last_chunk_tracking_view: Some(ChunkTrackingView::default()),
            load_queue: wants.iter().copied().collect(),
            ..Default::default()
        };
        world.spawn((observer, InDimension(dim)));
        dim
    }

    #[test]
    fn a_wanted_column_is_raised_without_waiting_for_it_to_land() {
        let mut world = World::new();
        let near = ColumnPos::new(0, 0);
        let middle = ColumnPos::new(0, 2);
        let far = ColumnPos::new(4, 0);
        one_player_looking_at_the_origin(&mut world, &[far, near, middle]);

        world
            .run_system_once(update_load_queue)
            .expect("the drain runs");

        assert_eq!(
            requests(&mut world),
            vec![near, middle, far],
            "nothing has landed, and all three are raised nearest first"
        );
    }

    /// Raising a column costs a whole column's worth of spawns. Counting the budget in
    /// sections would let the view ask for more than `spawn_chunks` can hand out, and the
    /// ticket queue would grow every tick without ever draining.
    #[test]
    fn the_view_never_raises_more_columns_than_a_tick_can_spawn() {
        let mut world = World::new();
        let sections_per_column = 16usize;
        let wants: Vec<ColumnPos> = (0..MAX_SPAWNS_PER_TICK as i32)
            .map(|x| ColumnPos::new(x, 0))
            .collect();
        one_player_looking_at_the_origin(&mut world, &wants);
        // The view reaches every one of them, so only the budget can hold it back.
        let mut observer = world
            .query::<&mut PlayerChunkObserver>()
            .single_mut(&mut world)
            .expect("one player");
        observer.last_last_chunk_tracking_view =
            Some(ChunkTrackingView::new(ChunkPos::new(0, 0, 0), u8::MAX, 8));

        world
            .run_system_once(update_load_queue)
            .expect("the drain runs");

        assert_eq!(
            requests(&mut world).len(),
            MAX_SPAWNS_PER_TICK / sections_per_column,
            "the budget is columns, sized so a tick's spawns can absorb them"
        );
    }
}
