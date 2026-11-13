use bevy_app::SubApp;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::ComponentId;
use bevy_ecs::message::{Message, Messages};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{ScheduleLabel, SystemSet};
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs::world::World;
use mcrs_engine_cell::{SubAppWorld, Tick};
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_engine_registries::RegistrySnapshot;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnTraceLog, TraceEvent};
use mcrs_minecraft_engine_world::world::storage::column::ColumnPosComponent;
use mcrs_minecraft_environment::WorldClocks;
use mcrs_voxel::{ColumnPos, SectionPos};

use crate::banlist::{BanList, EditBans};
use crate::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerReleased,
};
use crate::cell_mail;
use crate::cell_mail::{CellInbox, CellMail};
use crate::channel_types::ToDim;
use crate::column_production::ColumnProduction;
use crate::column_request::ColumnRequest;
use crate::column_store::{ColumnStoreInput, ColumnStoreLink, ColumnStoreOutput};
use crate::column_streams::ColumnRequests;
use crate::command_handoff::CommandHandoff;
use crate::coordinator::clock::{EditClock, GameTime};
use crate::coordinator::ops::{EditOps, OpList};
use crate::coordinator::running_dimensions::RunningDimensions;
use crate::departure::{Departure, PlayerTransfer};
use crate::dim_mail::DimMail;
use crate::hot_ticket::RemoteTicket;
use crate::persistence;
use crate::persistence::PlayerSaveQueue;
use crate::player_dimension_change;
use crate::player_order::OrderedPlayer;
use crate::player_roster::{ForeignPlayers, OwnPlayers, RosterRow};
use crate::server_edit::ServerEdit;
use crate::tile_events::TileEvents;
use crate::to_muxer::ClientOutput;
use crate::whitelist::{EditWhitelist, Whitelist};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::{CellEntityIds, EntityIdRange};
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::tick_pace::TickPace;

/// The host schedule that ticks the dimension worlds, right after `Last`: each world takes
/// in what the host sent during the update.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TickDimensions;

/// The two steps of [`TickDimensions`], in this order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickDimensionsSet {
    /// The muxer hands over the input it routed to each dimension during the update.
    Route,
    /// The coordinator ticks every dimension world with that input.
    Tick,
}

/// The system set of the inbox drain, early in `FixedPreUpdate`: the messages of the input of a
/// tick reach the dimension here. Systems that consume them run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DimInboxDrain;

/// A dimension world as the host ticks it.
pub type DimWorld = SubAppWorld<DimInput, DimOutput>;

/// What the host hands a dimension world for one tick. A copy of a host resource comes only
/// when the host changed it since the last tick; a new world gets every copy when it spawns.
pub struct DimInput {
    pub(crate) save: Option<crate::cold_columns::SaveRequest>,
    pub(crate) game_time: GameTime,
    pub(crate) birth_clock: BirthClock,
    pub(crate) clocks: Option<WorldClocks>,
    pub(crate) ops: Option<OpList>,
    pub(crate) whitelist: Option<Whitelist>,
    pub(crate) bans: Option<BanList>,
    pub(crate) game_rules: Option<GameRules>,
    pub(crate) tick_pace: Option<TickPace>,
    pub(crate) registries: Option<Versioned<RegistrySnapshot>>,
    pub(crate) dimensions: Option<RunningDimensions>,
    /// The players of the other worlds, in a tick whose input carries commands.
    pub(crate) players: Option<Vec<RosterRow>>,
    pub(crate) control: Vec<ToDim>,
    pub(crate) serverbound: Vec<InboundPlayerPacket>,
    /// The range that the coordinator grants for the request of the last tick.
    pub(crate) entity_id_grant: Option<EntityIdRange>,
    /// What the muxer asks of the version streams of the columns, in the order it asked.
    pub(crate) column_requests: Vec<ColumnRequest>,
    /// What the other cells of the world sent in their last tick, in the order of the cells.
    pub(crate) cell_mail: CellMail,
    /// The answers of the data layer of the world to the requests of this cell, and what the
    /// generation of the world made of the columns that this cell asked for.
    pub(crate) store_answers: Vec<ColumnStoreOutput>,
}

/// What a dimension world hands the host after one tick.
#[derive(Default)]
pub struct DimOutput {
    pub(crate) client: ClientOutput,
    pub(crate) traces: Vec<TraceEvent>,
    /// The players that left the dimension in the tick, after the packets of the tick.
    pub(crate) departures: Vec<Departure>,
    /// Whether the dimension holds few free entity ids and asks for a range.
    pub(crate) wants_entity_ids: bool,
    /// The players of the dimension at the end of the tick.
    pub(crate) roster: Vec<RosterRow>,
    /// The changes to the world clocks that the tick made.
    pub(crate) clock_edits: Vec<EditClock>,
    /// The changes to the whitelist that the tick made.
    pub(crate) whitelist_edits: Vec<EditWhitelist>,
    /// The changes to the ban list that the tick made.
    pub(crate) ban_edits: Vec<EditBans>,
    /// The changes to the operators that the tick made.
    pub(crate) ops_edits: Vec<EditOps>,
    /// The changes to the state of the whole server that the tick asked for.
    pub(crate) server_edits: Vec<ServerEdit>,
    /// What the tick hands the other cells of the world. A world of one cell hands nothing.
    pub(crate) cell_mail: CellMail,
    /// The requests of the tick to the data layer of the world, in order.
    pub(crate) store_inputs: Vec<ColumnStoreInput>,
    /// The columns that the tick asks the generation of the world for.
    pub(crate) generation_asks: Vec<ColumnPos>,
    /// Whether a player file failed to reach the disk since the last tick.
    pub(crate) player_save_failed: bool,
}

impl DimOutput {
    /// Appends what another cell of the same dimension handed over in the same tick. The traces,
    /// the entity id request, the roster, the messages between the cells and the requests to the
    /// data layer stay with each cell.
    pub(crate) fn merge(&mut self, other: Self) {
        self.client.append(other.client);
        self.departures.extend(other.departures);
        self.clock_edits.extend(other.clock_edits);
        self.whitelist_edits.extend(other.whitelist_edits);
        self.ban_edits.extend(other.ban_edits);
        self.ops_edits.extend(other.ops_edits);
        self.server_edits.extend(other.server_edits);
        self.player_save_failed |= other.player_save_failed;
    }
}

/// The messages of the input of this tick, until the inbox drain writes them at the sync point.
#[derive(Resource, Default)]
pub(crate) struct DimInbox {
    control: Vec<ToDim>,
    serverbound: Vec<InboundPlayerPacket>,
}

impl DimInbox {
    fn is_empty(&self) -> bool {
        self.control.is_empty() && self.serverbound.is_empty()
    }
}

pub(crate) fn dim_world(sub_app: SubApp) -> DimWorld {
    SubAppWorld::new(sub_app, apply_input, take_output).with_idle_check(is_idle)
}

/// Whether a tick would change nothing in the world: it holds no player, no ticket of another
/// cell and no section, and no input or answer waits in it. The copies of the host resources
/// that the input carries still land, and a later tick reacts to them.
fn is_idle(world: &mut World) -> bool {
    let busy: [Option<ComponentId>; 4] = [
        world.component_id::<Player>(),
        world.component_id::<RemoteTicket>(),
        world.component_id::<SectionPos>(),
        world.component_id::<ColumnPosComponent>(),
    ];
    let holds_busy = world.archetypes().iter().any(|archetype| {
        !archetype.is_empty() && busy.iter().flatten().any(|&id| archetype.contains(id))
    });
    !holds_busy
        && world
            .get_resource::<DimInbox>()
            .is_none_or(DimInbox::is_empty)
        && world
            .get_resource::<CellInbox>()
            .is_none_or(|inbox| inbox.0.is_empty())
        && world
            .get_resource::<ColumnRequests>()
            .is_none_or(|requests| requests.0.is_empty())
        && world
            .get_resource::<ColumnStoreLink>()
            .is_none_or(ColumnStoreLink::is_idle)
        && world
            .get_resource::<ColumnProduction>()
            .is_none_or(ColumnProduction::is_idle)
        && world
            .get_resource::<PlayerSaveQueue>()
            .is_none_or(PlayerSaveQueue::is_idle)
}

fn apply_input(world: &mut World, _tick: Tick, input: DimInput) {
    if let Some(request) = input.save {
        world.insert_resource(request);
    }
    if let Some(mut time) = world.get_resource_mut::<GameTime>() {
        time.set_if_neq(input.game_time);
    }
    world.insert_resource(input.birth_clock);
    if let Some(clocks) = input.clocks {
        world.insert_resource(clocks);
    }
    if let Some(ops) = input.ops {
        world.insert_resource(ops);
    }
    if let Some(whitelist) = input.whitelist {
        world.insert_resource(whitelist);
    }
    if let Some(bans) = input.bans {
        world.insert_resource(bans);
    }
    if let Some(game_rules) = input.game_rules {
        world.insert_resource(game_rules);
    }
    if let Some(tick_pace) = input.tick_pace {
        world.insert_resource(tick_pace);
    }
    if let Some(registries) = input.registries {
        mcrs_minecraft_engine_registries::apply_registries(world, registries);
    }
    if let Some(dimensions) = input.dimensions {
        world.insert_resource(dimensions);
    }
    if let Some(mut foreign) = world.get_resource_mut::<ForeignPlayers>() {
        foreign.0 = input.players.unwrap_or_default();
    }
    if let Some(range) = input.entity_id_grant
        && let Some(mut ids) = world.get_resource_mut::<CellEntityIds>()
    {
        ids.add(range);
    }
    if let Some(mut inbox) = world.get_resource_mut::<DimInbox>() {
        inbox.control.extend(input.control);
        inbox.serverbound.extend(input.serverbound);
    }
    if !input.cell_mail.is_empty()
        && let Some(mut inbox) = world.get_resource_mut::<CellInbox>()
    {
        let mut mail = input.cell_mail;
        inbox.0.append(&mut mail);
    }
    if !input.column_requests.is_empty()
        && let Some(mut requests) = world.get_resource_mut::<ColumnRequests>()
    {
        requests.0.extend(input.column_requests);
    }
    if !input.store_answers.is_empty()
        && let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>()
    {
        store.receive(input.store_answers);
    }
}

fn take_output(world: &mut World) -> DimOutput {
    let mut output = DimOutput::default();
    world.remove_resource::<crate::cold_columns::SaveRequest>();
    // After every schedule of the tick, so the player is in no system's view once it leaves.
    player_dimension_change::depart_players(world, &mut output.departures);
    cell_mail::take_cell_mail(world, &mut output.cell_mail);
    // The world the player arrives in writes the same file and starts its own writer, so a
    // write of this world that is still running must land first, or it would overwrite the
    // newer file.
    if !output.departures.is_empty() || !output.cell_mail.crossings.is_empty() {
        persistence::flush_player_saves(world);
    }
    drain_into(world, &mut output.client.packets);
    if let Some(mut released) = world.get_resource_mut::<Messages<OutboundPlayerReleased>>() {
        output
            .client
            .released
            .extend(released.drain().map(|released| released.session));
    }
    drain_into(world, &mut output.client.attached);
    output.player_save_failed = world
        .get_resource_mut::<PlayerSaveQueue>()
        .is_some_and(|mut queue| queue.take_failed());
    if let Some(log) = world.get_resource::<ColumnTraceLog>() {
        output.traces.extend(log.drain());
    }
    if let Some(mut events) = world.get_resource_mut::<TileEvents>() {
        output.client.tiles = events.take();
    }
    output.wants_entity_ids = world
        .get_resource::<CellEntityIds>()
        .is_some_and(CellEntityIds::wants_range);
    if let Some(mut own) = world.get_resource_mut::<OwnPlayers>() {
        output.roster = std::mem::take(&mut own.0);
        let departures = &output.departures;
        output.roster.retain(|row| {
            !departures
                .iter()
                .any(|departure| departure.session == row.session)
        });
    }
    if let Some(mut mail) = world.get_resource_mut::<DimMail>() {
        output.client.orders = std::mem::take(&mut mail.orders);
        output.clock_edits = std::mem::take(&mut mail.clock_edits);
        output.whitelist_edits = std::mem::take(&mut mail.whitelist_edits);
        output.ban_edits = std::mem::take(&mut mail.ban_edits);
        output.ops_edits = std::mem::take(&mut mail.ops_edits);
        output.server_edits = std::mem::take(&mut mail.server_edits);
        output.client.handoffs = std::mem::take(&mut mail.handoffs);
    }
    drain_into(world, &mut output.client.views);
    drain_into(world, &mut output.client.column_versions);
    drain_into(world, &mut output.client.column_changes);
    drain_into(world, &mut output.client.positions);
    drain_into(world, &mut output.client.block_acks);
    if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
        store.take_sent(&mut output.store_inputs);
        store.take_generation_asks(&mut output.generation_asks);
    }
    output
}

/// Moves every message of type `M` that the tick wrote into `out`.
fn drain_into<M: Message>(world: &mut World, out: &mut Vec<M>) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<M>>() {
        out.extend(messages.drain());
    }
}

/// Writes the messages of the input of this tick. The control messages go first, so a spawn
/// always precedes a serverbound packet of the same tick for the player it spawns.
pub(crate) fn drain_dim_inbox(
    mut inbox: ResMut<DimInbox>,
    mut spawns: ResMut<Messages<InboundPlayerSpawn>>,
    mut despawns: ResMut<Messages<InboundPlayerDespawn>>,
    mut serverbound: ResMut<Messages<InboundPlayerPacket>>,
    mut transfers: ResMut<Messages<PlayerTransfer>>,
    mut orders: ResMut<Messages<OrderedPlayer>>,
    mut handoffs: ResMut<Messages<CommandHandoff>>,
) {
    let inbox = &mut *inbox;
    for message in inbox.control.drain(..) {
        match message {
            ToDim::Spawn(spawn) => {
                spawns.write(spawn);
            }
            ToDim::Despawn(despawn) => {
                despawns.write(despawn);
            }
            ToDim::Transfer(transfer) => {
                transfers.write(*transfer);
            }
            ToDim::Order(order) => {
                orders.write(order);
            }
            ToDim::Command(handoff) => {
                handoffs.write(*handoff);
            }
        }
    }
    serverbound.write_batch(inbox.serverbound.drain(..));
}

pub(crate) fn inbox_holds_messages(inbox: Res<DimInbox>) -> bool {
    !inbox.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cold_columns::SaveRequest;

    fn input(save: Option<SaveRequest>) -> DimInput {
        DimInput {
            save,
            game_time: GameTime(0),
            birth_clock: BirthClock(0),
            clocks: None,
            ops: None,
            whitelist: None,
            bans: None,
            game_rules: None,
            tick_pace: None,
            registries: None,
            dimensions: None,
            players: None,
            control: Vec::new(),
            serverbound: Vec::new(),
            entity_id_grant: None,
            column_requests: Vec::new(),
            cell_mail: CellMail::default(),
            store_answers: Vec::new(),
        }
    }

    #[test]
    fn a_save_request_lasts_one_tick_in_a_world_without_a_column_store() {
        let mut world = World::new();
        apply_input(&mut world, Tick::default(), input(Some(SaveRequest::Dirty)));
        assert!(world.contains_resource::<SaveRequest>());

        take_output(&mut world);
        assert!(!world.contains_resource::<SaveRequest>());
    }

    #[test]
    fn a_departing_player_waits_for_the_running_write_of_its_file() {
        use crate::persistence::tests::{app, profile, temp_world};
        use mcrs_minecraft_engine_world::entity::EntityUuid;
        use mcrs_minecraft_engine_world::entity::physics::Transform;
        use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
        use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let dir = temp_world();
        let mut app = app(&dir);
        app.add_message::<ChangeDimensionRequest>();
        let world = app.world_mut();
        let player = world
            .spawn((
                Player,
                Owner(PlayerSession(1)),
                EntityUuid(profile().id),
                Transform::default(),
                crate::ability::PlayerGameMode::default(),
                crate::ability::PlayerFlying(false),
                crate::ability::PlayerOpLevel(0),
                crate::teleport::TeleportState::default(),
                profile(),
            ))
            .id();
        world.write_message(ChangeDimensionRequest {
            player,
            to: mcrs_minecraft_resource::ResourceKeyBuf::from_location(
                mcrs_minecraft_resource::ResourceLocationBuf::new("minecraft", "the_nether")
                    .unwrap(),
            ),
            transform: Transform::default(),
        });
        let (open, gate) = std::sync::mpsc::channel();
        let done = Arc::new(AtomicBool::new(false));
        world.insert_resource(PlayerSaveQueue::gated(gate, done.clone()));
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = open.send(());
        });

        let output = take_output(world);

        assert_eq!(output.departures.len(), 1);
        assert!(done.load(Ordering::SeqCst), "the write finished first");
        let _ = std::fs::remove_dir_all(dir);
    }
}
