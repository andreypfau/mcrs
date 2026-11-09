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
use mcrs_minecraft_engine_world::owned_columns::{HALO, column_key};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_environment::WorldClocks;
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;

use crate::banlist::BanList;
use crate::cell_mail::CellMail;
use crate::channel_types::ToDim;
use crate::column_generation::{GenerationAnswer, GenerationAsk};
use crate::column_request::ColumnRequest;
use crate::column_store::{ColumnDisk, ColumnStoreOutput};
use crate::command_handoff::CommandHandoff;
use crate::coordinator::birth_clock::BirthClock;
use crate::coordinator::clock::GameTime;
use crate::coordinator::entity_id_ranges::EntityIdRanges;
use crate::coordinator::ops::OpList;
use crate::coordinator::roster::PlayerRoster;
use crate::coordinator::running_dimensions::RunningDimensions;
use crate::departure::PlayerTransfer;
use crate::dim_tick;
use crate::dim_tick::{DimInput, DimOutput, DimWorld, TickDimensions, TickDimensionsSet};
use crate::entity_ids::EntityIdRange;
use crate::game_rules::GameRules;
use crate::player_roster::RosterRow;
use crate::player_view::PlayerView;
use crate::to_coordinator::RoutedInput;
use crate::to_muxer::{DimClosed, DimDelivered, DimOpened};
use crate::whitelist::Whitelist;
use mcrs_minecraft_engine_world::tick_pace::TickPace;

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
    dims: BTreeMap<Entity, Dim>,
    cells: BTreeMap<CellId, Cell>,
    /// The cell that holds the player of each session, in a dimension of several cells.
    sessions: FxHashMap<PlayerSession, CellId>,
    tick: Tick,
    outputs: Vec<DimTicked>,
}

/// A view of a player that a cell of the world of `label` reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportedView {
    pub label: Entity,
    pub view: PlayerView,
}

/// One dimension: its cells and the data layer that they share.
struct Dim {
    /// The cells, in the order of their index.
    cells: Vec<CellId>,
    /// The division of the columns over the cells, for a dimension of more than one cell.
    partition: Option<BoxPartition>,
    store: SharedStore<ColumnDisk, CellId>,
}

/// The place of a cell in its dimension, and what the other cells, the data layer and the
/// generation of its world sent it for its next tick.
struct Cell {
    label: Entity,
    index: CellIndex,
    inbox: CellMail,
    store_answers: Vec<ColumnStoreOutput>,
    generated: Vec<ColumnStoreOutput>,
}

impl Default for DimCells {
    fn default() -> Self {
        Self {
            worlds: SyncCell::new(Worlds::default()),
            dims: BTreeMap::new(),
            cells: BTreeMap::new(),
            sessions: FxHashMap::default(),
            tick: Tick::default(),
            outputs: Vec::new(),
        }
    }
}

impl Dim {
    fn new() -> Self {
        Self {
            cells: Vec::new(),
            partition: None,
            store: SharedStore::new(ColumnDisk::new(), Placement::Thread),
        }
    }

    fn cell_at(&self, index: CellIndex) -> Option<CellId> {
        let position = usize::try_from(index.0).ok()?;
        self.cells.get(position).copied()
    }

    /// The cell that owns `column`.
    fn owner(&self, column: ColumnPos) -> Option<CellId> {
        match self.partition {
            Some(partition) => self.cell_at(partition.owner(column_key(column))),
            None => self.cells.first().copied(),
        }
    }

    /// Whether the cell `index` reads `column`.
    fn reads(&self, index: CellIndex, column: ColumnPos) -> bool {
        self.partition
            .is_some_and(|partition| partition.reaches(index, column_key(column), HALO))
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
    /// Places the world of the cell `index` of the dimension of `label`. The cells of a
    /// dimension come in the order of their index.
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
        self.cells.insert(
            cell,
            Cell {
                label,
                index,
                inbox: CellMail::default(),
                store_answers: Vec::new(),
                generated: Vec::new(),
            },
        );
        self.dims
            .entry(label)
            .or_insert_with(Dim::new)
            .cells
            .push(cell);
        cell
    }

    /// Divides the dimension of `label` over its cells by `partition`.
    pub(crate) fn divide(&mut self, label: Entity, partition: BoxPartition) {
        self.dims.entry(label).or_insert_with(Dim::new).partition = Some(partition);
    }

    /// Stops every world of `label`. Returns `false` when no world has that label.
    pub(crate) fn remove(&mut self, label: Entity) -> bool {
        let Some(dim) = self.dims.remove(&label) else {
            return false;
        };
        let worlds = self.worlds.get();
        for cell in dim.cells {
            self.cells.remove(&cell);
            #[cfg(feature = "world-access")]
            worlds.lent.remove(&cell);
            block_on(worlds.cells.remove(cell));
        }
        let cells = &self.cells;
        self.sessions.retain(|_, cell| cells.contains_key(cell));
        true
    }

    /// The number of dimensions.
    #[cfg(feature = "world-access")]
    #[must_use]
    pub fn len(&self) -> usize {
        self.dims.len()
    }

    #[cfg(feature = "world-access")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dims.is_empty()
    }

    #[must_use]
    pub fn contains(&self, label: Entity) -> bool {
        self.dims.contains_key(&label)
    }

    /// The labels of the dimensions.
    #[cfg(feature = "world-access")]
    pub fn labels(&self) -> impl Iterator<Item = Entity> + '_ {
        self.dims.keys().copied()
    }

    /// The number of cells of the dimension of `label`.
    #[cfg(feature = "world-access")]
    #[must_use]
    pub fn cell_count(&self, label: Entity) -> usize {
        self.dims.get(&label).map_or(0, |dim| dim.cells.len())
    }

    /// The world of each cell of `label`, lent out of its cell until the next tick gives it
    /// back, in the order of the cells.
    ///
    /// For tests and tools only: the parts of the server send a dimension world messages and
    /// never read or write it between ticks.
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

    /// Every world, lent out as [`Self::cell_sub_apps_mut`] lends one, in the order they tick.
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
        for dim in self.dims.values_mut() {
            dim.store.answered().await;
        }
    }

    /// The columns that each cell asked the generation of its world for in the last tick, in
    /// the order of the cells.
    pub fn take_generation_asks(&mut self) -> Vec<GenerationAsk> {
        let mut asks = Vec::new();
        for ticked in &mut self.outputs {
            asks.extend(
                ticked
                    .output
                    .generation_asks
                    .drain(..)
                    .map(|column| GenerationAsk {
                        label: ticked.label,
                        cell: ticked.cell,
                        column,
                    }),
            );
        }
        asks
    }

    /// The views of the players that each cell reported in the last tick, with the label of the
    /// world of the cell.
    pub fn reported_views(&self) -> impl Iterator<Item = ReportedView> + '_ {
        self.outputs.iter().flat_map(|ticked| {
            ticked
                .output
                .client
                .views
                .iter()
                .map(move |view| ReportedView {
                    label: ticked.label,
                    view: *view,
                })
        })
    }

    /// Hands `answer` of the generation of its world to `cell`, in the input of its next tick.
    pub fn deliver_generated(&mut self, cell: CellId, answer: GenerationAnswer) {
        if let Some(cell) = self.cells.get_mut(&cell) {
            cell.generated.push(answer.into());
        }
    }

    pub(crate) fn take_outputs(&mut self) -> Vec<DimTicked> {
        mem::take(&mut self.outputs)
    }

    /// The cells of the dimension of `label`, in the order of their index.
    pub(crate) fn cells_of(&self, label: Entity) -> impl Iterator<Item = CellId> + '_ {
        self.dims
            .get(&label)
            .into_iter()
            .flat_map(|dim| dim.cells.iter().copied())
    }

    /// The cell of `label` that owns `column`.
    fn owner(&self, label: Entity, column: ColumnPos) -> Option<CellId> {
        self.dims.get(&label)?.owner(column)
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
            .or_else(|| self.cells_of(label).next())
    }

    /// Splits `handoff` by the owner of the column of each source, whose blocks the stage
    /// reads. A handoff with no source goes to the cell that holds the player who sent the line.
    fn split_handoff(
        &self,
        split: &mut BTreeMap<CellId, RoutedInput>,
        label: Entity,
        mut handoff: CommandHandoff,
    ) {
        let sources = mem::take(&mut handoff.sources);
        if sources.is_empty() {
            if let Some(cell) = self.holder(label, handoff.output) {
                let message = ToDim::Command(Box::new(handoff));
                part(split, label, cell).control.push(message);
            }
            return;
        }
        let mut parts: BTreeMap<CellId, CommandHandoff> = BTreeMap::new();
        for source in sources {
            if let Some(cell) = self.owner(label, ColumnPos::from(source.position)) {
                parts
                    .entry(cell)
                    .or_insert_with(|| handoff.clone())
                    .sources
                    .push(source);
            }
        }
        for (cell, handoff) in parts {
            let message = ToDim::Command(Box::new(handoff));
            part(split, label, cell).control.push(message);
        }
    }

    /// Splits what the muxer routed to the dimension of `routed.dim` by the cell that owns
    /// each part: a player goes to the cell that owns the column it arrives in, its packets to
    /// the cell that holds it, a column request to the owner of the column.
    fn split(&mut self, routed: RoutedInput) -> BTreeMap<CellId, RoutedInput> {
        let label = routed.dim;
        let mut split: BTreeMap<CellId, RoutedInput> = BTreeMap::new();
        let dim = self.dims.get(&label);
        for message in routed.control {
            let message = match message {
                ToDim::Command(handoff) => {
                    self.split_handoff(&mut split, label, *handoff);
                    continue;
                }
                message => message,
            };
            let cell = match &message {
                ToDim::Spawn(spawn) => dim.and_then(|dim| {
                    place(
                        dim,
                        &mut self.sessions,
                        spawn.session,
                        spawn.snapshot.position,
                    )
                }),
                ToDim::Transfer(transfer) => {
                    let (session, position) = match &**transfer {
                        PlayerTransfer::Arrive(departure) => {
                            (departure.session, departure.arrival.transform.translation)
                        }
                        PlayerTransfer::Return(departure) => {
                            (departure.session, departure.carried.transform.translation)
                        }
                    };
                    dim.and_then(|dim| place(dim, &mut self.sessions, session, position))
                }
                ToDim::Despawn(despawn) => self.holder(label, despawn.session),
                ToDim::Order(order) => self.holder(label, order.session),
                ToDim::Command(_) => None,
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

    /// Hands what the cell `from` sent the other cells of its dimension to them, for their next
    /// tick, and returns the players that went to another cell. A change of a block goes only to
    /// the cells that read its column.
    pub(crate) fn post(&mut self, from: CellId, mail: CellMail) -> Vec<CellMove> {
        let Self {
            dims,
            cells,
            sessions,
            ..
        } = self;
        let Some(dim) = cells.get(&from).and_then(|cell| dims.get(&cell.label)) else {
            return Vec::new();
        };
        let CellMail {
            tickets,
            halo,
            writes,
            crossings,
            messages,
            ledgers,
            views,
            holds,
            asks,
            answers,
        } = mail;
        for &to in &dim.cells {
            if to == from {
                continue;
            }
            let Some(cell) = cells.get_mut(&to) else {
                continue;
            };
            let inbox = &mut cell.inbox;
            inbox.tickets.extend_from_slice(&tickets);
            inbox.ledgers.extend_from_slice(&ledgers);
            let index = cell.index;
            inbox.halo.extend(
                halo.iter()
                    .filter(|write| dim.reads(index, ColumnPos::from(write.pos))),
            );
            inbox.views.extend_from_slice(&views);
            inbox.holds.extend_from_slice(&holds);
        }
        for ask in asks {
            if let Some(inbox) = inbox(cells, dim.owner(ask.column)) {
                inbox.asks.push(ask);
            }
        }
        for answer in answers {
            if let Some(inbox) = inbox(cells, dim.cell_at(answer.to)) {
                inbox.answers.push(answer);
            }
        }
        for write in writes {
            if let Some(inbox) = inbox(cells, dim.owner(ColumnPos::from(write.pos))) {
                inbox.writes.push(write);
            }
        }
        for message in messages {
            if let Some(inbox) = inbox(cells, dim.owner(ColumnPos::from(message.section()))) {
                inbox.messages.push(message);
            }
        }
        let mut moves = Vec::new();
        for crossing in crossings {
            let session = crossing.session;
            let Some(to) = place(
                dim,
                sessions,
                session,
                crossing.carried.transform.translation,
            ) else {
                continue;
            };
            moves.push(CellMove { session, from, to });
            if let Some(inbox) = inbox(cells, Some(to)) {
                inbox.crossings.push(crossing);
            }
        }
        moves
    }

    /// Forgets the cell of a player that left its dimension.
    pub(crate) fn forget_session(&mut self, session: PlayerSession) {
        self.sessions.remove(&session);
    }

    fn tick(&mut self, mut input: impl FnMut(CellId) -> DimInput) {
        for dim in self.dims.values_mut() {
            for answer in dim.store.answers() {
                if let Some(cell) = self.cells.get_mut(&answer.to) {
                    cell.store_answers.push(answer.output);
                }
            }
        }
        let worlds = self.worlds.get();
        #[cfg(feature = "world-access")]
        for (_, lent) in mem::take(&mut worlds.lent) {
            worlds.cells.give_back(lent);
        }
        let cells = &mut self.cells;
        let ticked = block_on(worlds.cells.tick(self.tick, |cell| {
            let mut input = input(cell);
            if let Some(cell) = cells.get_mut(&cell) {
                input.cell_mail = mem::take(&mut cell.inbox);
                input.store_answers = mem::take(&mut cell.store_answers);
                input.store_answers.append(&mut cell.generated);
            }
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
            let Some(cell) = self.cells.get(&ticked.cell) else {
                continue;
            };
            self.outputs.push(DimTicked {
                label: cell.label,
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
        for ticked in &mut self.outputs {
            let Some(dim) = self.dims.get_mut(&ticked.label) else {
                continue;
            };
            let puts = ticked
                .output
                .store_inputs
                .extract_if(.., |input| matches!(input, StoreInput::Put { .. }));
            for input in puts {
                dim.store.send(ticked.cell, input);
            }
        }
        for ticked in &mut self.outputs {
            let Some(dim) = self.dims.get_mut(&ticked.label) else {
                continue;
            };
            for input in mem::take(&mut ticked.output.store_inputs) {
                dim.store.send(ticked.cell, input);
            }
        }
    }
}

/// The cell of `dim` that a player of `session` arriving at `position` goes to. In a divided
/// dimension, it holds the player from now on.
fn place(
    dim: &Dim,
    sessions: &mut FxHashMap<PlayerSession, CellId>,
    session: PlayerSession,
    position: DVec3,
) -> Option<CellId> {
    let cell = dim.owner(ColumnPos::from(position))?;
    if dim.partition.is_some() {
        sessions.insert(session, cell);
    }
    Some(cell)
}

/// What the other cells sent `cell` for its next tick.
fn inbox(cells: &mut BTreeMap<CellId, Cell>, cell: Option<CellId>) -> Option<&mut CellMail> {
    cells.get_mut(&cell?).map(|cell| &mut cell.inbox)
}

/// The input of `cell` in `split`, empty until a part goes to it.
fn part(
    split: &mut BTreeMap<CellId, RoutedInput>,
    label: Entity,
    cell: CellId,
) -> &mut RoutedInput {
    split.entry(cell).or_insert_with(|| RoutedInput::new(label))
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
    birth_clock: BirthClock,
    clocks: Option<WorldClocks>,
    ops: Option<OpList>,
    whitelist: Option<Whitelist>,
    bans: Option<BanList>,
    game_rules: Option<GameRules>,
    tick_pace: Option<TickPace>,
    registries: Option<Versioned<RegistrySnapshot>>,
    dimensions: Option<RunningDimensions>,
}

impl Globals {
    fn read(host: &World) -> Self {
        Self {
            game_time: host.get_resource::<GameTime>().copied().unwrap_or_default(),
            birth_clock: host
                .get_resource::<BirthClock>()
                .copied()
                .unwrap_or_default(),
            clocks: changed(host),
            ops: changed(host),
            whitelist: changed(host),
            bans: changed(host),
            game_rules: changed(host),
            tick_pace: changed(host),
            registries: changed(host),
            dimensions: changed(host),
        }
    }

    fn input(
        &self,
        routed: Option<RoutedInput>,
        players: Option<Vec<RosterRow>>,
        entity_id_grant: Option<EntityIdRange>,
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
            birth_clock: self.birth_clock,
            clocks: self.clocks.clone(),
            ops: self.ops.clone(),
            whitelist: self.whitelist.clone(),
            bans: self.bans.clone(),
            game_rules: self.game_rules.clone(),
            tick_pace: self.tick_pace.clone(),
            registries: self.registries.clone(),
            dimensions: self.dimensions.clone(),
            players,
            control,
            serverbound,
            entity_id_grant,
            column_requests,
            cell_mail: CellMail::default(),
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
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_command::CommandLevel;
    use mcrs_minecraft_core::LookAnchor;
    use mcrs_minecraft_engine_world::owned_columns::region_partition;
    use std::num::NonZeroU32;

    use crate::command_handoff::SourceState;

    fn handoff(label: Entity, position: DVec3) -> ToDim {
        let source = SourceState {
            level: CommandLevel::All,
            player: None,
            mob: None,
            dimension: label,
            position,
            yaw: 0.0,
            pitch: 0.0,
            anchor: LookAnchor::Feet,
        };
        ToDim::Command(Box::new(CommandHandoff {
            to: label,
            output: PlayerSession(1),
            line: "execute if block ~ ~ ~ stone".into(),
            origin: source.clone(),
            stage: 1,
            sources: vec![source],
            cost: 0,
        }))
    }

    #[test]
    fn a_handoff_goes_to_the_cells_that_own_the_columns_of_its_sources() {
        let label = Entity::PLACEHOLDER;
        let mut cells = DimCells::default();
        let first = cells.add(label, CellIndex(0), SubApp::new(), Placement::Inline);
        let second = cells.add(label, CellIndex(1), SubApp::new(), Placement::Inline);
        let partition = region_partition(NonZeroU32::MIN.saturating_add(1));
        cells.divide(label, partition);
        let cell_of = |position: DVec3| match partition.owner(column_key(ColumnPos::from(position)))
        {
            CellIndex(0) => first,
            _ => second,
        };
        let west = DVec3::new(-8.0, 64.0, 8.0);
        let east = DVec3::new(8.0, 64.0, 8.0);
        assert_ne!(cell_of(west), cell_of(east), "x = 0 is a region boundary");

        for position in [west, east] {
            let split = cells.split(RoutedInput {
                dim: label,
                control: vec![handoff(label, position)],
                serverbound: Vec::new(),
                column_requests: Vec::new(),
            });
            let receivers: Vec<CellId> = split.keys().copied().collect();
            assert_eq!(
                receivers,
                [cell_of(position)],
                "the handoff at {position:?}"
            );
        }

        let ToDim::Command(mut both) = handoff(label, west) else {
            panic!("a command handoff");
        };
        both.sources.push(SourceState {
            position: east,
            ..both.origin.clone()
        });
        let split = cells.split(RoutedInput {
            dim: label,
            control: vec![ToDim::Command(both)],
            serverbound: Vec::new(),
            column_requests: Vec::new(),
        });
        for position in [west, east] {
            let sources: Vec<DVec3> = split
                .get(&cell_of(position))
                .into_iter()
                .flat_map(|input| &input.control)
                .filter_map(|message| match message {
                    ToDim::Command(handoff) => Some(handoff),
                    _ => None,
                })
                .flat_map(|handoff| handoff.sources.iter().map(|source| source.position))
                .collect();
            assert_eq!(sources, [position], "the owner of {position:?}");
        }
    }
}
