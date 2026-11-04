use std::collections::BTreeMap;
use std::mem;

use bevy_app::{App, Last, MainScheduleOrder, Plugin, SubApp};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use bevy_platform::cell::SyncCell;
use bevy_tasks::block_on;
use mcrs_engine_cell::{CellId, Cells, Lent, Placement, Tick};
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_engine_registries::RegistrySnapshot;
use mcrs_minecraft_environment::world_clock::WorldClocks;

use crate::coordinator::clock::GameTime;
use crate::coordinator::ops::OpList;
use crate::dim_tick;
use crate::dim_tick::{DimInput, DimOutput, DimWorld, TickDimensions, TickDimensionsSet};
use crate::to_coordinator::RoutedInput;
use crate::to_muxer::{DimClosed, DimDelivered, DimOpened};
use rustc_hash::FxHashMap;

/// Ticks the dimension worlds once in every host update, in lockstep.
pub struct DimCellsPlugin;

impl Plugin for DimCellsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimCells>();
        app.add_message::<DimOpened>();
        app.add_message::<DimClosed>();
        app.add_message::<DimDelivered>();
        app.add_message::<RoutedInput>();
        if let Some(mut order) = app.world_mut().get_resource_mut::<MainScheduleOrder>() {
            order.insert_after(Last, TickDimensions);
        }
        app.configure_sets(
            TickDimensions,
            (TickDimensionsSet::Route, TickDimensionsSet::Tick).chain(),
        );
        app.add_systems(
            TickDimensions,
            tick_dimensions.in_set(TickDimensionsSet::Tick),
        );
    }
}

/// The dimension worlds of the host, each a cell, by the label entity of its dimension.
#[derive(Resource)]
pub struct DimCells {
    // A world holds a `SubApp`, which is `Send` but not `Sync`. Only `&mut` reaches it.
    worlds: SyncCell<Worlds>,
    labels: BTreeMap<CellId, Entity>,
    tick: Tick,
    outputs: Vec<DimTicked>,
}

impl Default for DimCells {
    fn default() -> Self {
        Self {
            worlds: SyncCell::new(Worlds::default()),
            labels: BTreeMap::new(),
            tick: Tick::default(),
            outputs: Vec::new(),
        }
    }
}

#[derive(Default)]
struct Worlds {
    cells: Cells<DimWorld>,
    lent: BTreeMap<CellId, Lent<DimWorld>>,
}

/// The output of the dimension of `label` for one tick.
pub(crate) struct DimTicked {
    pub(crate) label: Entity,
    pub(crate) output: DimOutput,
}

impl DimCells {
    pub(crate) fn add(&mut self, label: Entity, sub_app: SubApp, placement: Placement) {
        let cell = self
            .worlds
            .get()
            .cells
            .add(dim_tick::dim_world(sub_app), placement);
        self.labels.insert(cell, label);
    }

    /// Stops the world of `label`. Returns `false` when no world has that label.
    pub(crate) fn remove(&mut self, label: Entity) -> bool {
        let Some(cell) = self.cell_of(label) else {
            return false;
        };
        self.labels.remove(&cell);
        let worlds = self.worlds.get();
        worlds.lent.remove(&cell);
        block_on(worlds.cells.remove(cell));
        true
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    #[must_use]
    pub fn contains(&self, label: Entity) -> bool {
        self.cell_of(label).is_some()
    }

    /// The labels of the dimensions, in the order they tick.
    pub fn labels(&self) -> impl Iterator<Item = Entity> + '_ {
        self.labels.values().copied()
    }

    /// The world of `label`, lent out of its cell until the next tick gives it back.
    ///
    /// Transitional: only the command pass and tests read or write a dimension world between
    /// ticks this way, until commands become messages to the world.
    pub fn sub_app_mut(&mut self, label: Entity) -> Option<&mut SubApp> {
        let cell = self.cell_of(label)?;
        let worlds = self.worlds.get();
        lend(worlds, cell);
        worlds
            .lent
            .get_mut(&cell)
            .map(|lent| lent.world_mut().sub_app_mut())
    }

    /// Every world, lent out as [`Self::sub_app_mut`] lends one, in the order they tick.
    pub fn sub_apps_mut(&mut self) -> impl Iterator<Item = &mut SubApp> {
        let worlds = self.worlds.get();
        for &cell in self.labels.keys() {
            lend(worlds, cell);
        }
        worlds
            .lent
            .values_mut()
            .map(|lent| lent.world_mut().sub_app_mut())
    }

    pub(crate) fn take_outputs(&mut self) -> Vec<DimTicked> {
        mem::take(&mut self.outputs)
    }

    fn cell_of(&self, label: Entity) -> Option<CellId> {
        self.labels
            .iter()
            .find(|(_, found)| **found == label)
            .map(|(&cell, _)| cell)
    }

    fn tick(&mut self, mut input: impl FnMut(Entity) -> DimInput) {
        let worlds = self.worlds.get();
        let labels = &self.labels;
        for (_, lent) in mem::take(&mut worlds.lent) {
            worlds.cells.give_back(lent);
        }
        let ticked = block_on(worlds.cells.tick(self.tick, |cell| {
            input(labels.get(&cell).copied().unwrap_or(Entity::PLACEHOLDER))
        }));
        self.tick = Tick(self.tick.0.wrapping_add(1));
        let outputs = match ticked {
            Ok(outputs) => outputs,
            Err(error) => {
                tracing::error!(%error, "the dimensions did not tick");
                return;
            }
        };
        for ticked in outputs {
            let Some(&label) = self.labels.get(&ticked.cell) else {
                continue;
            };
            self.outputs.push(DimTicked {
                label,
                output: ticked.output,
            });
        }
    }
}

fn lend(worlds: &mut Worlds, cell: CellId) {
    if worlds.lent.contains_key(&cell) {
        return;
    }
    if let Some(lent) = block_on(worlds.cells.lend(cell)) {
        worlds.lent.insert(cell, lent);
    }
}

/// The copies of the host resources that every dimension gets in the input of this tick.
struct Globals {
    game_time: GameTime,
    clocks: Option<WorldClocks>,
    ops: Option<OpList>,
    registries: Option<Versioned<RegistrySnapshot>>,
}

impl Globals {
    fn read(host: &World) -> Self {
        Self {
            game_time: host.get_resource::<GameTime>().copied().unwrap_or_default(),
            clocks: changed(host),
            ops: changed(host),
            registries: changed(host),
        }
    }

    fn input(&self, routed: Option<RoutedInput>) -> DimInput {
        let (control, serverbound) = routed.map_or_else(Default::default, |routed| {
            (routed.control, routed.serverbound)
        });
        DimInput {
            game_time: self.game_time,
            clocks: self.clocks.clone(),
            ops: self.ops.clone(),
            registries: self.registries.clone(),
            control,
            serverbound,
        }
    }
}

fn changed<R: Resource + Clone>(host: &World) -> Option<R> {
    host.get_resource_ref::<R>()
        .filter(|resource| resource.is_changed())
        .map(|resource| R::clone(&*resource))
}

/// Ticks every dimension world once with the copies of the host resources and the input that
/// the muxer routed during the update, and keeps the outputs for the host to take between ticks.
pub fn tick_dimensions(world: &mut World) {
    let globals = Globals::read(world);
    let mut routed: FxHashMap<Entity, RoutedInput> = world
        .get_resource_mut::<Messages<RoutedInput>>()
        .map(|mut messages| {
            messages
                .drain()
                .map(|routed| (routed.dim, routed))
                .collect()
        })
        .unwrap_or_default();
    if let Some(mut cells) = world.get_resource_mut::<DimCells>() {
        cells.tick(|label| globals.input(routed.remove(&label)));
    }
}
