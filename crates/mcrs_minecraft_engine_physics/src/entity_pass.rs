use std::mem;

use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine::player_tiles::TILE_WIDTH;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::KeyedStream;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TileRows;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::same_bits;
use mcrs_minecraft_physics::tick_entity;
use mcrs_minecraft_replication::MobState;
use mcrs_voxel::SectionPos;

use crate::entity_rows::EntityRows;
use crate::entity_rows::Leaving;
use crate::entity_rows::Tile;
use crate::entity_rows::wakes;
use crate::front_grid::FrontGrid;
use crate::front_grid::grid_row;
use crate::mob_intent::MobIntent;
use crate::mob_life::Placed;
use crate::mob_moved::MobMoved;
use crate::tile_at::tile_at;
use crate::tile_world::TileWorld;

/// The age at which an item or an experience orb despawns (AIT-VAN-018, AIT-VAN-019).
const LIFETIME: u32 = 6000;
/// The death time at which a dying entity leaves the world (DMG-DTH-003).
const DEATH_TICKS: u8 = 20;

const SERVER: Side = Side::Server { authority: true };

/// The frozen inputs of one entity pass.
pub(crate) struct Pass<'a, W> {
    pub(crate) world: &'a W,
    pub(crate) tables: &'a Tables,
    /// The tick of the keyed random streams.
    pub(crate) tick: u64,
    pub(crate) push: PushRules,
    pub(crate) tiles_per_task: usize,
}

/// What the task of one tile hands back.
struct TileStep {
    tile: TileKey,
    /// The largest half width of the bodies that the task wrote into the back buffer of the tile;
    /// `None` when no body of the tile changed and the task wrote nothing.
    reach: Option<f32>,
    expired: Vec<EntityId>,
    /// The rows whose position is in another tile now.
    left: Vec<EntityId>,
    moved: Vec<MobMoved>,
    /// The living rows of `expired`.
    died: Vec<EntityId>,
}

impl EntityRows {
    /// Ticks every row once, in parallel over tiles: each row reads only its own state, the
    /// blocks of `pass` and the front grid, so the result does not depend on the task split or
    /// the thread count. Writes the grid of each tile whose bodies changed into its back buffer,
    /// and keeps the rows that left their tile for [`Self::move_rows`].
    /// Gives the items and the orbs that reached their lifetime, which left their rows.
    pub(crate) fn tick<W: CollisionWorld + Sync>(
        &mut self,
        pass: &Pass<'_, W>,
        pool: &TaskPool,
        grid: &BodyGrid,
    ) -> Vec<EntityId> {
        self.moved.clear();
        self.removed.clear();
        let bodies = FrontGrid {
            grid,
            reach: self.reach,
        };
        let woken = &self.woken;
        let mut tiles: Vec<(TileKey, &mut Tile)> = self
            .tiles
            .iter_mut()
            .map(|(&tile, entry)| (tile, entry))
            .collect();
        let steps: Vec<Vec<TileStep>> = pool.scope(|scope| {
            for chunk in tiles.chunks_mut(pass.tiles_per_task.max(1)) {
                scope.spawn(async move {
                    let mut scratch = Scratch::new();
                    chunk
                        .iter_mut()
                        .map(|(tile, entry)| {
                            let wake = wakes(woken, *tile);
                            tick_tile(pass, &bodies, *tile, entry, wake, &mut scratch)
                        })
                        .collect()
                });
            }
        });
        let mut expired = Vec::new();
        for step in steps.into_iter().flatten() {
            if let Some(reach) = step.reach {
                self.written.insert(step.tile);
                self.reach = self.reach.max(reach);
            }
            self.removed.extend(step.died.into_iter().map(|id| Placed {
                tile: step.tile,
                id,
            }));
            expired.extend(step.expired);
            self.moved.extend(step.moved);
            self.leaving.extend(step.left.into_iter().map(|id| Leaving {
                from: step.tile,
                id,
            }));
        }
        expired
    }
}

fn tick_tile<W: CollisionWorld, B: BodyView>(
    pass: &Pass<'_, W>,
    bodies_view: &B,
    tile: TileKey,
    entry: &mut Tile,
    wake: bool,
    scratch: &mut Scratch,
) -> TileStep {
    let rows = &mut entry.rows;
    let world = tile_world(pass.world, tile, rows);
    let view = View::new(&world, bodies_view, pass.tables);
    let mut actions = Vec::new();
    let mut bodies = Vec::new();
    let mut stirred = false;
    let mut expired = Vec::new();
    let mut left = Vec::new();
    let mut moved = Vec::new();
    let mut died = Vec::new();
    let mut intents = mem::take(&mut entry.intents);
    intents.sort_by_key(|intent| intent.target);
    let rules = pass.push;
    for row in rows.rows_mut() {
        let state = row.state;
        let looked = state.living.map(rotations);
        if let Some(living) = state.living.as_mut() {
            if !living.is_alive() {
                living.steering = None;
                living.input = MoveInput::default();
                living.jump = false;
            } else if let Ok(at) = intents.binary_search_by_key(&row.id, |intent| intent.target)
                && let Some(intent) = intents.get(at)
            {
                apply_intent(intent, living);
            }
        }
        let start = grid_row(state, row.motion, row.id);
        if loaded(view.world, state.body.position) {
            let before = state.body;
            tick_entity(view, SERVER, row.motion, state, wake, &mut actions, scratch);
            actions.clear();
            if let Some(own) = start {
                let stream = KeyedStream::new(&state.key, pass.tick);
                let pushed = push_entity(
                    view.bodies,
                    &own,
                    &rules,
                    &stream,
                    &mut state.body.velocity,
                    scratch,
                );
                let crowded = rules.max_entity_cramming > 0
                    && i64::from(pushed.candidates) > i64::from(rules.max_entity_cramming) - 1;
                if pushed.pushed || crowded {
                    state.rest = Rest::Awake;
                }
                stirred |= !same_bits(&before, &state.body);
            }
            if let Some(looked) = looked
                && (!same_bits(&before, &state.body) || state.living.map(rotations) != Some(looked))
            {
                moved.push(MobMoved {
                    id: row.id,
                    state: mob_state(state),
                });
            }
            state.age = state.age.wrapping_add(1);
            let expires = matches!(
                row.motion.category,
                Category::Item | Category::ExperienceOrb
            );
            let dead = state.living.as_mut().is_some_and(count_down);
            if state.living.is_some_and(|living| {
                living.hurt_time > 0 || living.damage_cooldown > 0 || !living.is_alive()
            }) {
                state.rest = Rest::Awake;
            }
            if dead || (expires && state.age >= LIFETIME) {
                if dead {
                    died.push(row.id);
                }
                expired.push(row.id);
            } else if tile_at(state.body.position) != tile {
                left.push(row.id);
            }
        }
        bodies.extend(grid_row(state, row.motion, row.id));
    }
    for &id in &expired {
        rows.despawn_row(id);
    }
    TileStep {
        tile,
        reach: stirred.then(|| entry.write(tile, &bodies)),
        expired,
        left,
        moved,
        died,
    }
}

/// The damage counters and the death time of one tick (DMG-HIT-006, DMG-DTH-003). Gives true
/// when the death time reaches its end and the entity leaves the world.
fn count_down(living: &mut Living) -> bool {
    living.hurt_time = living.hurt_time.saturating_sub(1);
    living.damage_cooldown = living.damage_cooldown.saturating_sub(1);
    let Some(death_time) = living.death_time.as_mut() else {
        return false;
    };
    *death_time = death_time.saturating_add(1);
    *death_time >= DEATH_TICKS
}

/// The rotations of a living row that its viewers see, as bits.
fn rotations(living: Living) -> [u32; 3] {
    [living.yaw, living.pitch, living.head_yaw].map(f32::to_bits)
}

fn apply_intent(intent: &MobIntent, living: &mut Living) {
    if intent.steering.is_some() {
        living.steering = intent.steering;
    }
    let turns = [
        (intent.yaw, &mut living.yaw),
        (intent.pitch, &mut living.pitch),
        (intent.head_yaw, &mut living.head_yaw),
        (intent.body_yaw, &mut living.body_yaw),
    ];
    for (turn, angle) in turns {
        if let Some(turn) = turn {
            *angle = turn;
        }
    }
}

/// The state of a living row that its viewers see.
pub(crate) fn mob_state(state: &EntityState) -> MobState {
    let living = state.living.unwrap_or_default();
    MobState {
        position: state.body.position,
        velocity: state.body.velocity,
        yaw: living.yaw,
        pitch: living.pitch,
        head_yaw: living.head_yaw,
        on_ground: state.body.flags.on_ground,
        health: living.health,
        dying: !living.is_alive(),
    }
}

/// The sections of the tile and one section around it, from below the lowest row to above the
/// highest one.
fn tile_world<'a, W: CollisionWorld>(
    world: &'a W,
    tile: TileKey,
    rows: &mut TileRows,
) -> TileWorld<'a, W> {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for row in rows.rows_mut() {
        low = low.min(row.state.body.position.y);
        high = high.max(row.state.body.position.y);
    }
    let sections = TILE_WIDTH.get().cast_signed() / 16;
    let bottom = SectionPos::from(DVec3::new(0.0, low - 4.0, 0.0)).0.y;
    let top = SectionPos::from(DVec3::new(0.0, high + 4.0, 0.0)).0.y;
    TileWorld::new(
        world,
        SectionPos::new(
            tile.x.saturating_mul(sections) - 1,
            bottom,
            tile.z.saturating_mul(sections) - 1,
        ),
        [sections + 2, top - bottom + 1, sections + 2],
    )
}

/// True when the section of `position` is loaded or outside the world: a row in a section that
/// is not loaded does not tick.
fn loaded<W: CollisionWorld>(world: &W, position: DVec3) -> bool {
    !matches!(
        world.section(SectionPos::from(block_at(position))),
        SectionLookup::Unloaded
    )
}

#[cfg(test)]
mod tests {
    use bevy_tasks::TaskPoolBuilder;

    use super::*;
    use crate::EntityWrites;
    use crate::TestScene;

    #[test]
    fn an_intent_that_only_turns_the_head_publishes_a_move() {
        let pool = TaskPoolBuilder::new().num_threads(2).build();
        let mut scene = TestScene::new(1, 2, 0, 7);
        let standing = EntityId(1);
        for _ in 0..60 {
            scene.step(&pool, 1, &mut EntityWrites::default());
        }
        assert!(
            scene.rows.moved.iter().all(|moved| moved.id != standing),
            "the standing walker settled on the floor"
        );
        let tile = scene
            .rows
            .living_rows()
            .find(|(at, _)| at.id == standing)
            .map(|(at, _)| at.tile)
            .expect("the standing walker has a row");
        scene.rows.steer(
            tile,
            MobIntent {
                head_yaw: Some(45.0),
                ..MobIntent::new(standing)
            },
        );
        scene.step(&pool, 1, &mut EntityWrites::default());
        assert!(
            scene.rows.moved.iter().any(|moved| moved.id == standing),
            "the turn of the head reaches the viewers"
        );
    }
}
