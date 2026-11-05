use std::collections::BTreeMap;
use std::mem;

use bevy_app::{App, Last, MainScheduleOrder, Plugin, SubApp};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bevy_platform::cell::SyncCell;
use bevy_tasks::block_on;
use mcrs_engine_cell::{BoxPartition, CellId, CellIndex, Cells, Placement, Tick};
use mcrs_engine_store::{SharedStore, StoreInput};
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_engine_registries::RegistrySnapshot;
use mcrs_minecraft_engine_world::owned_columns::column_key;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_environment::world_clock::WorldClocks;
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;

use crate::cell_mail::{CellMail, CellMessage};
use crate::channel_types::ToDim;
use crate::column_request::ColumnRequest;
use crate::column_store::{ColumnDisk, ColumnStoreInput, ColumnStoreOutput};
use crate::coordinator::clock::GameTime;
use crate::coordinator::entity_id_ranges::EntityIdRanges;
use crate::coordinator::ops::OpList;
use crate::coordinator::roster::PlayerRoster;
use crate::coordinator::running_dimensions::RunningDimensions;
use crate::departure::PlayerTransfer;
use crate::dim_tick;
use crate::dim_tick::{DimInput, DimOutput, DimWorld, TickDimensions, TickDimensionsSet};
use crate::entity_ids::EntityIdRange;
use crate::player_roster::RosterRow;
use crate::to_coordinator::RoutedInput;
use crate::to_muxer::{DimClosed, DimDelivered, DimOpened};

/// Ticks the dimension worlds once in every host update, in lockstep.
pub struct DimCellsPlugin;

impl Plugin for DimCellsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimCells>();
        app.init_resource::<EntityIdRanges>();
        app.init_resource::<PlayerRoster>();
        app.init_resource::<RunningDimensions>();
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

/// The dimension worlds of the host, by the label entity of their dimension. A dimension is one
/// cell, or several cells that each own the columns of a static box of its regions.
#[derive(Resource)]
pub struct DimCells {
    // A world holds a `SubApp`, which is `Send` but not `Sync`. Only `&mut` reaches it.
    worlds: SyncCell<Worlds>,
    cells: BTreeMap<CellId, CellPlace>,
    /// The division of each dimension that has more than one cell.
    partitions: BTreeMap<Entity, BoxPartition>,
    /// The cell that holds the player of each session, in a dimension of several cells.
    sessions: FxHashMap<PlayerSession, CellId>,
    /// The messages between the cells of one dimension, for their next tick.
    mail: BTreeMap<CellId, Vec<CellMessage>>,
    /// The data layer of each dimension, which all its cells share.
    stores: BTreeMap<Entity, SharedStore<ColumnDisk, CellId>>,
    tick: Tick,
    outputs: Vec<DimTicked>,
}

/// The dimension of a cell and the place of the cell in its dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellPlace {
    label: Entity,
    index: CellIndex,
}

impl Default for DimCells {
    fn default() -> Self {
        Self {
            worlds: SyncCell::new(Worlds::default()),
            cells: BTreeMap::new(),
            partitions: BTreeMap::new(),
            sessions: FxHashMap::default(),
            mail: BTreeMap::new(),
            stores: BTreeMap::new(),
            tick: Tick::default(),
            outputs: Vec::new(),
        }
    }
}

#[derive(Default)]
struct Worlds {
    cells: Cells<DimWorld>,
    #[cfg(feature = "world-access")]
    lent: BTreeMap<CellId, mcrs_engine_cell::Lent<DimWorld>>,
}

/// The output of the cell `cell` of the dimension of `label` for one tick.
pub(crate) struct DimTicked {
    pub(crate) label: Entity,
    pub(crate) cell: CellId,
    pub(crate) output: DimOutput,
}

/// A player that went from the cell `from` to the cell `to` of one dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CellMove {
    pub(crate) session: PlayerSession,
    pub(crate) from: CellId,
    pub(crate) to: CellId,
}

impl DimCells {
    /// Places the world of the cell `index` of the dimension of `label`.
    pub(crate) fn add(
        &mut self,
        label: Entity,
        index: CellIndex,
        sub_app: SubApp,
        placement: Placement,
    ) -> CellId {
        let cell = self
            .worlds
            .get()
            .cells
            .add(dim_tick::dim_world(sub_app), placement);
        self.cells.insert(cell, CellPlace { label, index });
        self.stores
            .entry(label)
            .or_insert_with(|| SharedStore::new(ColumnDisk::new(), Placement::Thread));
        cell
    }

    /// Divides the dimension of `label` over its cells by `partition`.
    pub(crate) fn divide(&mut self, label: Entity, partition: BoxPartition) {
        self.partitions.insert(label, partition);
    }

    /// Stops every world of `label`. Returns `false` when no world has that label.
    pub(crate) fn remove(&mut self, label: Entity) -> bool {
        let cells: Vec<CellId> = self.cells_of(label).collect();
        if cells.is_empty() {
            return false;
        }
        self.partitions.remove(&label);
        self.stores.remove(&label);
        let worlds = self.worlds.get();
        for cell in cells {
            self.cells.remove(&cell);
            self.mail.remove(&cell);
            #[cfg(feature = "world-access")]
            worlds.lent.remove(&cell);
            block_on(worlds.cells.remove(cell));
        }
        let cells = &self.cells;
        self.sessions.retain(|_, cell| cells.contains_key(cell));
        true
    }

    /// The number of dimensions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.labels().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    #[must_use]
    pub fn contains(&self, label: Entity) -> bool {
        self.home(label).is_some()
    }

    /// The labels of the dimensions, in the order they tick.
    pub fn labels(&self) -> impl Iterator<Item = Entity> + '_ {
        let mut seen: Vec<Entity> = Vec::new();
        self.cells.values().filter_map(move |place| {
            if seen.contains(&place.label) {
                return None;
            }
            seen.push(place.label);
            Some(place.label)
        })
    }

    /// The number of cells of the dimension of `label`.
    #[must_use]
    pub fn cell_count(&self, label: Entity) -> usize {
        self.cells_of(label).count()
    }

    /// The world of the first cell of `label`, lent out of its cell until the next tick gives
    /// it back.
    ///
    /// For tests and tools only: the parts of the server send a dimension world messages and
    /// never read or write it between ticks.
    #[cfg(feature = "world-access")]
    pub fn sub_app_mut(&mut self, label: Entity) -> Option<&mut SubApp> {
        let cell = self.home(label)?;
        let worlds = self.worlds.get();
        lend(worlds, cell);
        worlds
            .lent
            .get_mut(&cell)
            .map(|lent| lent.world_mut().sub_app_mut())
    }

    /// The world of each cell of `label`, lent out as [`Self::sub_app_mut`] lends one, in the
    /// order of the cells.
    #[cfg(feature = "world-access")]
    pub fn cell_sub_apps_mut(&mut self, label: Entity) -> impl Iterator<Item = &mut SubApp> {
        let cells: Vec<CellId> = self.cells_of(label).collect();
        let worlds = self.worlds.get();
        for &cell in &cells {
            lend(worlds, cell);
        }
        worlds
            .lent
            .iter_mut()
            .filter(move |(cell, _)| cells.contains(cell))
            .map(|(_, lent)| lent.world_mut().sub_app_mut())
    }

    /// Every world, lent out as [`Self::sub_app_mut`] lends one, in the order they tick.
    #[cfg(feature = "world-access")]
    pub fn sub_apps_mut(&mut self) -> impl Iterator<Item = &mut SubApp> {
        let worlds = self.worlds.get();
        for &cell in self.cells.keys() {
            lend(worlds, cell);
        }
        worlds
            .lent
            .values_mut()
            .map(|lent| lent.world_mut().sub_app_mut())
    }

    /// Waits until the data layer of every dimension has answered every request sent so far.
    ///
    /// For tests and tools only, to make the answers land at the same tick in every run.
    #[cfg(feature = "world-access")]
    pub async fn stores_answered(&mut self) {
        for store in self.stores.values_mut() {
            store.answered().await;
        }
    }

    pub(crate) fn take_outputs(&mut self) -> Vec<DimTicked> {
        mem::take(&mut self.outputs)
    }

    pub(crate) fn cells_of(&self, label: Entity) -> impl Iterator<Item = CellId> + '_ {
        self.cells
            .iter()
            .filter(move |(_, place)| place.label == label)
            .map(|(&cell, _)| cell)
    }

    fn home(&self, label: Entity) -> Option<CellId> {
        self.cells_of(label).next()
    }

    fn cell_at(&self, label: Entity, index: CellIndex) -> Option<CellId> {
        self.cells
            .iter()
            .find(|(_, place)| **place == CellPlace { label, index })
            .map(|(&cell, _)| cell)
    }

    /// The cell of `label` that owns `column`.
    fn owner(&self, label: Entity, column: ColumnPos) -> Option<CellId> {
        match self.partitions.get(&label) {
            Some(partition) => self.cell_at(label, partition.owner(column_key(column))),
            None => self.home(label),
        }
    }

    /// The cell of `label` that holds the player of `session`, or the first cell.
    fn holder(&self, label: Entity, session: PlayerSession) -> Option<CellId> {
        self.sessions
            .get(&session)
            .copied()
            .filter(|cell| {
                self.cells
                    .get(cell)
                    .is_some_and(|place| place.label == label)
            })
            .or_else(|| self.home(label))
    }

    /// Splits what the muxer routed to the dimension of `routed.dim` by the cell that owns
    /// each part: a player goes to the cell that owns the column it arrives in, its packets to
    /// the cell that holds it, a column request to the owner of the column.
    fn split(&mut self, routed: RoutedInput) -> BTreeMap<CellId, RoutedInput> {
        let label = routed.dim;
        let mut split: BTreeMap<CellId, RoutedInput> = BTreeMap::new();
        for message in routed.control {
            let cell = match &message {
                ToDim::Spawn(spawn) => self.place(label, spawn.session, spawn.snapshot.position),
                ToDim::Transfer(transfer) => {
                    let (session, position) = match &**transfer {
                        PlayerTransfer::Arrive(departure) => {
                            (departure.session, departure.arrival.transform.translation)
                        }
                        PlayerTransfer::Return(departure) => {
                            (departure.session, departure.carried.transform.translation)
                        }
                    };
                    self.place(label, session, position)
                }
                ToDim::Despawn(despawn) => self.holder(label, despawn.session),
                ToDim::Order(order) => self.holder(label, order.session),
                ToDim::Command(_) => self.home(label),
            };
            if let Some(cell) = cell {
                part(&mut split, label, cell).control.push(message);
            }
        }
        for packet in routed.serverbound {
            if let Some(cell) = self.holder(label, packet.session) {
                part(&mut split, label, cell).serverbound.push(packet);
            }
        }
        for request in routed.column_requests {
            let column = match request {
                ColumnRequest::Want(column) | ColumnRequest::Drop(column) => column,
            };
            if let Some(cell) = self.owner(label, column) {
                part(&mut split, label, cell).column_requests.push(request);
            }
        }
        split
    }

    /// The cell that a player of `session` arriving at `position` goes to. It holds the player
    /// from now on.
    fn place(&mut self, label: Entity, session: PlayerSession, position: DVec3) -> Option<CellId> {
        let cell = self.owner(label, ColumnPos::from(position))?;
        if self.partitions.contains_key(&label) {
            self.sessions.insert(session, cell);
        }
        Some(cell)
    }

    /// Hands what the cell `from` sent the other cells of its dimension to them, for their next
    /// tick, and returns the players that went to another cell.
    pub(crate) fn post(&mut self, from: CellId, mail: CellMail) -> Vec<CellMove> {
        let Some(&CellPlace { label, .. }) = self.cells.get(&from) else {
            return Vec::new();
        };
        let others: Vec<CellId> = self.cells_of(label).filter(|cell| *cell != from).collect();
        let CellMail {
            tickets,
            halo,
            writes,
            crossings,
        } = mail;
        for &cell in &others {
            let inbox = self.mail.entry(cell).or_default();
            inbox.extend(tickets.iter().copied().map(CellMessage::Ticket));
            inbox.extend(halo.iter().copied().map(CellMessage::Halo));
        }
        for write in writes {
            if let Some(cell) = self.owner(label, ColumnPos::from(write.pos)) {
                self.mail
                    .entry(cell)
                    .or_default()
                    .push(CellMessage::Write(write));
            }
        }
        let mut moves = Vec::new();
        for crossing in crossings {
            let session = crossing.session();
            let Some(to) = self.place(label, session, crossing.position()) else {
                continue;
            };
            moves.push(CellMove { session, from, to });
            self.mail
                .entry(to)
                .or_default()
                .push(CellMessage::Cross(Box::new(crossing)));
        }
        moves
    }

    /// Forgets the cell of a player that left its dimension.
    pub(crate) fn forget_session(&mut self, session: PlayerSession) {
        self.sessions.remove(&session);
    }

    fn tick(&mut self, mut input: impl FnMut(CellId) -> DimInput) {
        let mut answers: BTreeMap<CellId, Vec<ColumnStoreOutput>> = BTreeMap::new();
        for store in self.stores.values_mut() {
            for answer in store.answers() {
                answers.entry(answer.to).or_default().push(answer.output);
            }
        }
        let worlds = self.worlds.get();
        #[cfg(feature = "world-access")]
        for (_, lent) in mem::take(&mut worlds.lent) {
            worlds.cells.give_back(lent);
        }
        let ticked = block_on(worlds.cells.tick(self.tick, |cell| {
            let mut input = input(cell);
            input.store_answers = answers.remove(&cell).unwrap_or_default();
            input
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
            let Some(&CellPlace { label, .. }) = self.cells.get(&ticked.cell) else {
                continue;
            };
            self.outputs.push(DimTicked {
                label,
                cell: ticked.cell,
                output: ticked.output,
            });
        }
        self.send_to_stores();
    }

    /// Sends the requests of the cells of this tick to the data layer of their dimension: the
    /// writes of all cells first, so a cell that reads a column in the tick in which its owner
    /// wrote it sees the write, then the other requests, each cell in its order.
    fn send_to_stores(&mut self) {
        let mut rest: Vec<ColumnStoreInput> = Vec::new();
        for ticked in &mut self.outputs {
            let Some(store) = self.stores.get_mut(&ticked.label) else {
                continue;
            };
            for input in mem::take(&mut ticked.output.store_inputs) {
                match input {
                    StoreInput::Put { .. } => store.send(ticked.cell, input),
                    StoreInput::Get { .. } | StoreInput::Flush => rest.push(input),
                }
            }
            ticked.output.store_inputs = mem::take(&mut rest);
        }
        for ticked in &mut self.outputs {
            let Some(store) = self.stores.get_mut(&ticked.label) else {
                continue;
            };
            for input in mem::take(&mut ticked.output.store_inputs) {
                store.send(ticked.cell, input);
            }
        }
    }
}

/// The input of `cell` in `split`, empty until a part goes to it.
fn part(
    split: &mut BTreeMap<CellId, RoutedInput>,
    label: Entity,
    cell: CellId,
) -> &mut RoutedInput {
    split.entry(cell).or_insert_with(|| RoutedInput {
        dim: label,
        control: Vec::new(),
        serverbound: Vec::new(),
        column_requests: Vec::new(),
    })
}

#[cfg(feature = "world-access")]
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
    dimensions: Option<RunningDimensions>,
}

impl Globals {
    fn read(host: &World) -> Self {
        Self {
            game_time: host.get_resource::<GameTime>().copied().unwrap_or_default(),
            clocks: changed(host),
            ops: changed(host),
            registries: changed(host),
            dimensions: changed(host),
        }
    }

    fn input(
        &self,
        routed: Option<RoutedInput>,
        players: Option<Vec<RosterRow>>,
        entity_id_grant: Option<EntityIdRange>,
        cell_mail: Vec<CellMessage>,
    ) -> DimInput {
        let (control, serverbound, column_requests) = match routed {
            Some(RoutedInput {
                control,
                serverbound,
                column_requests,
                ..
            }) => (control, serverbound, column_requests),
            None => Default::default(),
        };
        DimInput {
            game_time: self.game_time,
            clocks: self.clocks.clone(),
            ops: self.ops.clone(),
            registries: self.registries.clone(),
            dimensions: self.dimensions.clone(),
            players,
            control,
            serverbound,
            entity_id_grant,
            column_requests,
            cell_mail,
            store_answers: Vec::new(),
        }
    }
}

fn changed<R: Resource + Clone>(host: &World) -> Option<R> {
    host.get_resource_ref::<R>()
        .filter(|resource| resource.is_changed())
        .map(|resource| R::clone(&*resource))
}

/// Ticks every dimension world once with the copies of the host resources and the input that
/// the muxer routed during the update, split by the cell that owns each part, with the entity id
/// ranges granted after the last tick, the messages between the cells of a dimension and, for a
/// cell that runs commands, the players of the other cells. It keeps the outputs for the host
/// to take between ticks.
pub fn tick_dimensions(world: &mut World) {
    let globals = Globals::read(world);
    let routed: Vec<RoutedInput> = world
        .get_resource_mut::<Messages<RoutedInput>>()
        .map(|mut messages| messages.drain().collect())
        .unwrap_or_default();
    let mut granted = world
        .get_resource_mut::<EntityIdRanges>()
        .map(|mut ranges| ranges.take_granted())
        .unwrap_or_default();
    let Some(mut cells) = world.get_resource_mut::<DimCells>() else {
        return;
    };
    let mut split: BTreeMap<CellId, RoutedInput> = BTreeMap::new();
    for routed in routed {
        split.append(&mut cells.split(routed));
    }
    let mut mail = mem::take(&mut cells.mail);
    let mut players: FxHashMap<CellId, Vec<RosterRow>> = world
        .get_resource::<PlayerRoster>()
        .map(|roster| {
            split
                .iter()
                .filter(|(_, routed)| routed.carries_commands())
                .map(|(&cell, _)| (cell, roster.others(cell)))
                .collect()
        })
        .unwrap_or_default();
    if let Some(mut cells) = world.get_resource_mut::<DimCells>() {
        cells.tick(|cell| {
            globals.input(
                split.remove(&cell),
                players.remove(&cell),
                granted.remove(&cell),
                mail.remove(&cell).unwrap_or_default(),
            )
        });
    }
}
