use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, Res, SystemParam};
use glam::DVec3;
use mcrs_minecraft_collision_table::BlockWorld;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_engine::SavedSpawn;
use mcrs_minecraft_engine::coordinator::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine::coordinator::running_dimensions::{RunningDimension, RunningDimensions};
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnPos};
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_heightmap::{MotionHeightmap, SolidHeightmap, SurfaceHeightmap};
use mcrs_voxel::BlockPos;

use crate::spawn_blocks::{ColumnHeights, SpawnBlocks, lower_face_centre};

const MAX_SEARCH_RADIUS: u32 = 15;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Respawn {
    pub(crate) dimension: Entity,
    pub(crate) transform: Transform,
    pub(crate) search: SpawnSearch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpawnSearch {
    Done,
    InTarget,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ArrivedSpawn {
    Elsewhere,
    Waiting,
    At(Transform),
}

#[derive(SystemParam)]
pub(crate) struct SpawnSite<'w, 's> {
    saved: Option<Res<'w, SavedSpawn>>,
    running: Option<Res<'w, RunningDimensions>>,
    own: Option<Res<'w, DimensionLabel>>,
    default_mode: Option<Res<'w, DefaultGameMode>>,
    rules: Option<Res<'w, GameRules>>,
    tables: Option<Res<'w, PhysicsTables>>,
    dimension: Query<
        'w,
        's,
        (Entity, &'static DimensionTypeConfig, &'static ColumnIndex),
        With<Dimension>,
    >,
    world: WorldView<'w, 's>,
    columns: Query<
        'w,
        's,
        (
            &'static MotionHeightmap,
            &'static SurfaceHeightmap,
            &'static SolidHeightmap,
        ),
    >,
}

impl SpawnSite<'_, '_> {
    pub(crate) fn world_spawn(&self) -> Option<Respawn> {
        let saved = self.saved.as_deref();
        let target = self.respawn_dimension(saved)?;
        let spawn = spawn_block(saved);
        let (translation, search) = if self.is_own(target) {
            (self.adjusted(spawn), SpawnSearch::Done)
        } else {
            (lower_face_centre(spawn), SpawnSearch::InTarget)
        };
        Some(Respawn {
            dimension: target.label,
            transform: Transform {
                translation,
                rotation: spawn_rotation(saved),
            },
            search,
        })
    }

    pub(crate) fn arrived_spawn(&self) -> ArrivedSpawn {
        let saved = self.saved.as_deref();
        let Some(target) = self.respawn_dimension(saved) else {
            return ArrivedSpawn::Elsewhere;
        };
        if !self.is_own(target) {
            return ArrivedSpawn::Elsewhere;
        }
        let spawn = spawn_block(saved);
        if !self.is_loaded(spawn) {
            return ArrivedSpawn::Waiting;
        }
        ArrivedSpawn::At(Transform {
            translation: self.adjusted(spawn),
            rotation: spawn_rotation(saved),
        })
    }

    fn is_own(&self, dimension: &RunningDimension) -> bool {
        self.own.as_deref().map(|own| own.0) == Some(dimension.label)
    }

    fn is_loaded(&self, spawn: BlockPos) -> bool {
        let Ok((_, _, index)) = self.dimension.single() else {
            return false;
        };
        let radius = i32::try_from(self.search_radius()).unwrap_or(0);
        let xs = [
            spawn.x.saturating_sub(radius),
            spawn.x.saturating_add(radius),
        ];
        let zs = [
            spawn.z.saturating_sub(radius),
            spawn.z.saturating_add(radius),
        ];
        xs.into_iter()
            .all(|x| zs.into_iter().all(|z| self.heights(index, x, z).is_some()))
    }

    fn respawn_dimension(&self, saved: Option<&SavedSpawn>) -> Option<&RunningDimension> {
        let running = self.running.as_deref()?;
        let wanted = running.iter().find(|found| match saved {
            Some(saved) => found.key == saved.0.dimension,
            None => found.key == OVERWORLD,
        });
        wanted.or_else(|| running.iter().find(|found| found.key == OVERWORLD))
    }

    fn adjusted(&self, spawn: BlockPos) -> DVec3 {
        let centre = lower_face_centre(spawn);
        let mode = self
            .default_mode
            .as_deref()
            .map_or(GameMode::default(), |mode| mode.0);
        if mode == GameMode::Spectator {
            return centre;
        }
        let Ok((dimension, config, index)) = self.dimension.single() else {
            return centre;
        };
        let (Some(tables), Some(view)) = (self.tables.as_deref(), self.world.get(dimension)) else {
            return centre;
        };
        let blocks = SpawnBlocks {
            world: BlockWorld {
                blocks: &view,
                table: tables.collision(),
            },
            tables: tables.tables(),
            min_y: config.min_y,
            max_y: config.max_y(),
        };
        if mode == GameMode::Adventure {
            return blocks.height_fix(spawn);
        }
        let radius = self.search_radius();
        square(radius)
            .find_map(|(dx, dz)| {
                let x = spawn.x.checked_add(dx)?;
                let z = spawn.z.checked_add(dz)?;
                blocks.column(x, z, self.heights(index, x, z)?)
            })
            .unwrap_or_else(|| blocks.height_fix(spawn))
    }

    fn search_radius(&self) -> u32 {
        let radius = self
            .rules
            .as_deref()
            .map_or(0, |rules| rules.get(GameRule::RespawnRadius));
        u32::try_from(radius).unwrap_or(0).min(MAX_SEARCH_RADIUS)
    }

    fn heights(&self, index: &ColumnIndex, x: i32, z: i32) -> Option<ColumnHeights> {
        let column = index.get(&ColumnPos::from(BlockPos::new(x, 0, z)))?;
        let (motion, surface, floor) = self.columns.get(column.entity).ok()?;
        let local_x = usize::try_from(x & 15).ok()?;
        let local_z = usize::try_from(z & 15).ok()?;
        Some(ColumnHeights {
            motion: motion.0.get(local_x, local_z),
            surface: surface.0.get(local_x, local_z),
            ocean_floor: floor.0.get(local_x, local_z),
        })
    }
}

fn spawn_block(saved: Option<&SavedSpawn>) -> BlockPos {
    saved.map_or(BlockPos::new(0, 0, 0), |saved| {
        let [x, y, z] = saved.0.pos;
        BlockPos::new(x, y, z)
    })
}

fn spawn_rotation(saved: Option<&SavedSpawn>) -> Rotation {
    saved.map_or(Rotation::new(0.0, 0.0), |saved| {
        Rotation::new(saved.0.yaw, saved.0.pitch)
    })
}

fn square(radius: u32) -> impl Iterator<Item = (i32, i32)> {
    let side = radius * 2 + 1;
    let count = side * side;
    let start = radius * side + radius;
    let offset = i32::try_from(radius).unwrap_or(0);
    (0..count).filter_map(move |step| {
        let index = (start + step) % count;
        let dx = i32::try_from(index % side).ok()? - offset;
        let dz = i32::try_from(index / side).ok()? - offset;
        Some((dx, dz))
    })
}
