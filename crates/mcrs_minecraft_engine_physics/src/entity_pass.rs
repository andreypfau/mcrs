use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine::player_tiles::TILE_WIDTH;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::KeyedStream;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TileRows;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::same_bits;
use mcrs_minecraft_physics::tick_entity;
use mcrs_voxel::SectionPos;

use crate::entity_rows::EntityRows;
use crate::entity_rows::Leaving;
use crate::entity_rows::wakes;
use crate::front_grid::FrontGrid;
use crate::front_grid::grid_row;
use crate::front_grid::sort_bodies;
use crate::tile_at::tile_at;
use crate::tile_world::TileWorld;

/// The age at which an item or an experience orb despawns (AIT-VAN-018, AIT-VAN-019).
const LIFETIME: u32 = 6000;

const SERVER: Side = Side::Server { authority: true };

/// The frozen inputs of one entity pass.
pub struct Pass<'a, W> {
    pub world: &'a W,
    pub tables: &'a Tables,
    /// The tick of the keyed random streams.
    pub tick: u64,
    pub rules: PushRules,
    pub tiles_per_task: usize,
}

/// What the task of one tile hands back.
struct TileStep {
    tile: TileKey,
    /// The grid of the tile when one of its bodies changed.
    bodies: Option<Vec<GridRow>>,
    expired: Vec<EntityId>,
    /// The rows whose position is in another tile now.
    left: Vec<EntityId>,
}

impl EntityRows {
    /// Ticks every row once, in parallel over tiles: each row reads only its own state, the
    /// blocks of `pass` and the front grid, so the result does not depend on the task split or
    /// the thread count. Writes the grid of each tile whose bodies changed into its back buffer, and keeps the rows
    /// that left their tile for [`Self::move_rows`].
    /// Gives the items and the orbs that reached their lifetime, which left their rows.
    pub fn tick<W: CollisionWorld + Sync>(
        &mut self,
        pass: &Pass<'_, W>,
        pool: &TaskPool,
        grid: &mut BodyGrid,
    ) -> Vec<EntityId> {
        let bodies = FrontGrid {
            grid,
            reach: self.reach,
        };
        let woken = &self.woken;
        let mut tiles: Vec<(TileKey, &mut TileRows)> = self
            .tiles
            .iter_mut()
            .map(|(&tile, rows)| (tile, rows))
            .collect();
        let steps: Vec<Vec<TileStep>> = pool.scope(|scope| {
            for chunk in tiles.chunks_mut(pass.tiles_per_task.max(1)) {
                scope.spawn(async move {
                    let mut scratch = Scratch::new();
                    chunk
                        .iter_mut()
                        .map(|(tile, rows)| {
                            let wake = wakes(woken, *tile);
                            tick_tile(pass, &bodies, *tile, rows, wake, &mut scratch)
                        })
                        .collect()
                });
            }
        });
        let mut expired = Vec::new();
        for step in steps.into_iter().flatten() {
            if let Some(bodies) = step.bodies {
                self.install(grid, step.tile, bodies);
                self.stirred.insert(step.tile);
            }
            expired.extend(step.expired);
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
    rows: &mut TileRows,
    wake: bool,
    scratch: &mut Scratch,
) -> TileStep {
    let world = tile_world(pass.world, tile, rows);
    let view = View::new(&world, bodies_view, pass.tables);
    let mut actions = Vec::new();
    let mut bodies = Vec::new();
    let mut stirred = false;
    let mut expired = Vec::new();
    let mut left = Vec::new();
    for row in rows.rows_mut() {
        let state = row.state;
        let start = grid_row(state, row.motion, row.id);
        if loaded(view.world, state.body.position) {
            let before = state.body;
            tick_entity(view, SERVER, row.motion, state, wake, &mut actions, scratch);
            actions.clear();
            if let Some(own) = start {
                let stream = KeyedStream::new(&state.key, pass.tick);
                let rules = &pass.rules;
                let pushed = push_entity(
                    view.bodies,
                    &own,
                    rules,
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
            state.age = state.age.wrapping_add(1);
            let expires = matches!(
                row.motion.category,
                Category::Item | Category::ExperienceOrb
            );
            if expires && state.age >= LIFETIME {
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
        bodies: stirred.then(|| {
            sort_bodies(tile, &mut bodies);
            bodies
        }),
        expired,
        left,
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
        world.section(SectionPos::from(position)),
        SectionLookup::Unloaded
    )
}
