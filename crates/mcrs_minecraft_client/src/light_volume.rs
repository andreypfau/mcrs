use std::ops::RangeInclusive;
use std::sync::{Arc, LazyLock};

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::light::block_light_registry;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{ColumnPos, SectionPos};
use mcrs_minecraft_light::block::LightRegistry;
use mcrs_minecraft_light::level::LightBounds;
use mcrs_minecraft_light_color::colors::LightColors;
use mcrs_minecraft_light_color::layout::{
    Emitter, PackedBrick, lanes, neighbours, pack, reaching_types,
};
use mcrs_minecraft_light_color::region::section_bricks;
use mcrs_minecraft_render::{CameraOrigin, RenderPath, ShownPath};
use mcrs_minecraft_render_deferred::{VolumeCommand, VolumeQueue};

use crate::columns::{
    AIR, BlockSource, ClientTerrainSet, Column, ColumnChange, ColumnStore, Extent, SECTION_VOLUME,
};

static BRICKS_PER_FRAME: LazyLock<usize> = LazyLock::new(crate::config::color_bricks);

static AIR_SECTION: [u16; SECTION_VOLUME] = [AIR; SECTION_VOLUME];

pub struct LightVolumePlugin;

impl Plugin for LightVolumePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightVolumeFeed>().add_systems(
            Update,
            feed_light_volume
                .in_set(ClientTerrainSet::Build)
                .run_if(resource_exists::<ColumnStore>),
        );
    }
}

/// What the feeder has handed the renderer and what it still owes it, kept only while the
/// deferred path is shown. The column store stays the truth.
#[derive(Resource, Default)]
pub struct LightVolumeFeed {
    active: bool,
    generation: u64,
    emitters: HashMap<ColumnPos, Vec<i32>>,
    built: HashMap<IVec3, Vec<Emitter>>,
    to_build: HashSet<IVec3>,
    building: HashMap<IVec3, Task<PackedBrick>>,
    dirty: HashSet<IVec3>,
}

/// Bricks reach one column past the coloured radius, so every coloured section has all 27 of
/// its neighbours built.
#[derive(Clone, Copy, Debug)]
struct Scope {
    camera: IVec3,
    radius: i32,
    extent: Extent,
}

impl Scope {
    fn columns_from_camera(&self, column: ColumnPos) -> i32 {
        (column.x - self.camera.x)
            .abs()
            .max((column.z - self.camera.z).abs())
    }

    fn max_y(&self) -> i32 {
        self.extent.min_section_y + self.extent.sections as i32 - 1
    }

    fn brick_rows(&self) -> RangeInclusive<i32> {
        self.extent.min_section_y - 1..=self.max_y()
    }

    fn colours(&self, section: IVec3) -> bool {
        self.columns_from_camera(column_of(section)) <= self.radius
            && (self.extent.min_section_y..=self.max_y()).contains(&section.y)
    }
}

fn column_of(section: IVec3) -> ColumnPos {
    ColumnPos::new(section.x, section.z)
}

fn emitter_rows(column: &Column, registry: &LightRegistry) -> Vec<i32> {
    column
        .sections()
        .filter_map(|(y, section)| {
            section?
                .states
                .iter()
                .any(|&state| !registry.emission(VoxelId(state)).is_zero())
                .then_some(y)
        })
        .collect()
}

/// The rows of `column` whose brick is wanted: some emitter section lies within two sections of
/// them on every axis.
fn wanted_rows<'a>(
    scope: &Scope,
    column: ColumnPos,
    loaded: impl Fn(ColumnPos) -> bool,
    emitters: impl Fn(ColumnPos) -> Option<&'a [i32]>,
) -> Vec<i32> {
    if !loaded(column) || scope.columns_from_camera(column) > scope.radius + 1 {
        return Vec::new();
    }
    let mut near = Vec::new();
    for dz in -2..=2 {
        for dx in -2..=2 {
            let around = ColumnPos::new(column.x + dx, column.z + dz);
            near.extend(emitters(around).into_iter().flatten().copied());
        }
    }
    scope
        .brick_rows()
        .filter(|y| near.iter().any(|emitter| (emitter - y).abs() <= 2))
        .collect()
}

/// A brick holds the east, up and south vetoes of its own cells, so a column's arrival changes
/// the bricks of its west and north neighbours as well as its own.
fn rebuilt_by(at: ColumnPos) -> [ColumnPos; 3] {
    [
        at,
        ColumnPos::new(at.x - 1, at.z),
        ColumnPos::new(at.x, at.z - 1),
    ]
}

fn dirtied_by(at: ColumnPos, extent: Extent) -> impl Iterator<Item = IVec3> {
    let rows = extent.min_section_y - 1..=extent.min_section_y + extent.sections as i32;
    rows.flat_map(move |y| {
        (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| IVec3::new(at.x + dx, y, at.z + dz)))
    })
}

/// A loaded column's empty section is air; only a missing column is unloaded.
fn cells<'a, S: BlockSource>(
    source: &'a S,
) -> impl Fn(SectionPos) -> Option<&'a [u16; SECTION_VOLUME]> + 'a {
    move |at| {
        let column = source.column(at.x, at.z)?;
        Some(
            column
                .section(at.y)
                .map_or(&AIR_SECTION, |section| &*section.blocks),
        )
    }
}

impl LightVolumeFeed {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn arrive(
        &mut self,
        at: ColumnPos,
        emitter_rows: Vec<i32>,
        scope: &Scope,
        loaded: impl Fn(ColumnPos) -> bool,
    ) {
        let lit = !emitter_rows.is_empty();
        if lit {
            self.emitters.insert(at, emitter_rows);
        } else {
            self.emitters.remove(&at);
        }
        let stale = rebuilt_by(at);
        for dz in -2..=2 {
            for dx in -2..=2 {
                let column = ColumnPos::new(at.x + dx, at.z + dz);
                let rebuilt = stale.contains(&column);
                if !rebuilt && !lit {
                    continue;
                }
                let rows = wanted_rows(scope, column, &loaded, |around| {
                    self.emitters.get(&around).map(Vec::as_slice)
                });
                for y in rows {
                    let section = IVec3::new(column.x, y, column.z);
                    if rebuilt || !self.built.contains_key(&section) {
                        self.to_build.insert(section);
                    }
                }
            }
        }
        for section in dirtied_by(at, scope.extent) {
            if scope.colours(section) && loaded(column_of(section)) {
                self.dirty.insert(section);
            }
        }
    }

    fn depart(&mut self, at: ColumnPos) {
        self.emitters.remove(&at);
        self.to_build.retain(|&section| column_of(section) != at);
        self.building.retain(|&section, _| column_of(section) != at);
    }

    fn observe(
        &mut self,
        change: &ColumnChange,
        store: &ColumnStore,
        scope: &Scope,
        rows: impl Fn(&Column) -> Vec<i32>,
    ) {
        match change {
            ColumnChange::Arrived(at, _) => {
                if let Some(column) = store.get(*at) {
                    self.arrive(*at, rows(column), scope, |c| store.holds(c));
                }
            }
            ColumnChange::Departed(at, _) => self.depart(*at),
            ColumnChange::Relit(..) => {}
        }
    }

    fn land(&mut self, queue: &VolumeQueue) {
        let built = &mut self.built;
        self.building
            .retain(|&section, task| match check_ready(task) {
                Some(brick) => {
                    queue.push(VolumeCommand::Brick {
                        section,
                        words: brick.words,
                    });
                    built.insert(section, brick.emitters);
                    false
                }
                None => true,
            });
    }

    fn admit(
        &mut self,
        scope: &Scope,
        store: &ColumnStore,
        registry: &Arc<LightRegistry>,
        colours: &LightColors,
        per_frame: usize,
    ) {
        let mut next: Vec<IVec3> = self
            .to_build
            .iter()
            .copied()
            .filter(|section| !self.building.contains_key(section))
            .collect();
        if next.len() > per_frame {
            next.select_nth_unstable_by_key(per_frame, |&section| {
                (section - scope.camera).length_squared()
            });
            next.truncate(per_frame);
        }
        let bounds = LightBounds::new(scope.extent.min_section_y, scope.max_y());
        let pool = AsyncComputeTaskPool::get();
        for section in next {
            self.to_build.remove(&section);
            let column = column_of(section);
            if !store.holds(column) {
                continue;
            }
            let world = store.around(column);
            let (registry, colours) = (registry.clone(), colours.clone());
            let task = pool.spawn(async move {
                let at = SectionPos(section);
                pack(&section_bricks(
                    at,
                    bounds,
                    &registry,
                    &colours,
                    cells(&world),
                ))
            });
            self.building.insert(section, task);
        }
    }

    /// Dirty sections none of whose 27 neighbours still has a brick to build.
    fn take_ready(&mut self) -> Vec<IVec3> {
        let pending = self.to_build.len() + self.building.len();
        let ready: Vec<IVec3> = if pending == 0 {
            self.dirty.iter().copied().collect()
        } else if pending * 27 < self.dirty.len() {
            let blocked: HashSet<IVec3> = self
                .to_build
                .iter()
                .chain(self.building.keys())
                .flat_map(|&section| neighbours(SectionPos(section)).map(|n| n.0))
                .collect();
            self.dirty
                .iter()
                .copied()
                .filter(|section| !blocked.contains(section))
                .collect()
        } else {
            self.dirty
                .iter()
                .copied()
                .filter(|&section| {
                    neighbours(SectionPos(section))
                        .all(|n| !self.to_build.contains(&n.0) && !self.building.contains_key(&n.0))
                })
                .collect()
        };
        for section in &ready {
            self.dirty.remove(section);
        }
        ready
    }

    fn push_dirty(&mut self, colours: &LightColors, queue: &VolumeQueue) {
        for section in self.take_ready() {
            let types = reaching_types(SectionPos(section), |n| {
                self.built.get(&n.0).map(Vec::as_slice)
            });
            queue.push(VolumeCommand::Dirty {
                section,
                lanes: lanes(&types, colours),
            });
        }
    }
}

fn feed_light_volume(
    mut feed: ResMut<LightVolumeFeed>,
    mut changes: MessageReader<ColumnChange>,
    store: Res<ColumnStore>,
    shown: Res<ShownPath>,
    queue: Res<VolumeQueue>,
    origin: Res<CameraOrigin>,
    colours: Option<Res<LightColors>>,
    blocks: Option<Res<Blocks>>,
) {
    let radius = queue.radius();
    let (Some(colours), Some(blocks), Some(extent), RenderPath::Deferred, 1..) =
        (colours, blocks, store.extent(), shown.get(), radius)
    else {
        if feed.active {
            feed.clear();
        }
        return;
    };
    let feed = &mut *feed;
    let registry = block_light_registry(&blocks);
    let rows = |column: &Column| emitter_rows(column, &registry);
    let scope = Scope {
        camera: origin.section,
        radius: radius as i32,
        extent,
    };
    let generation = queue.generation();
    if !feed.active || feed.generation != generation {
        changes.clear();
        feed.clear();
        feed.active = true;
        feed.generation = generation;
        queue.push(VolumeCommand::Reset {
            min_section_y: extent.min_section_y,
            sections: extent.sections as u32,
        });
        let mut resident: Vec<ColumnPos> = store.resident().map(|(at, _)| at).collect();
        resident.sort_unstable_by_key(|&at| scope.columns_from_camera(at));
        for at in resident {
            if let Some(column) = store.get(at) {
                feed.arrive(at, rows(column), &scope, |c| store.holds(c));
            }
        }
    } else {
        for change in changes.read() {
            feed.observe(change, &store, &scope, rows);
        }
    }
    feed.land(&queue);
    feed.admit(&scope, &store, &registry, &colours, *BRICKS_PER_FRAME);
    feed.push_dirty(&colours, &queue);
}
