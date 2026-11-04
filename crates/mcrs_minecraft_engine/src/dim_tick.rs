use bevy_app::SubApp;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{ScheduleLabel, SystemSet};
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs::world::World;
use mcrs_engine_cell::{SubAppWorld, Tick};
use mcrs_engine_muxer::TileDeltas;
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_engine_registries::RegistrySnapshot;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnTraceLog, TraceEvent};
use mcrs_minecraft_environment::world_clock::WorldClocks;
use mcrs_minecraft_replication::PlayerData;

use crate::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerPacket, OutboundPlayerReleased,
};
use crate::channel_types::ToDim;
use crate::command_handoff::CommandHandoff;
use crate::coordinator::clock::{EditClock, GameTime};
use crate::coordinator::ops::OpList;
use crate::coordinator::running_dimensions::RunningDimensions;
use crate::departure::{Departure, PlayerTransfer};
use crate::dim_mail::DimMail;
use crate::entity_ids::{CellEntityIds, EntityIdRange, EntityIdRequest};
use crate::player_dimension_change;
use crate::player_order::OrderedPlayer;
use crate::player_roster::{ForeignPlayers, OwnPlayers, RosterRow};
use crate::tile_events::TileEvents;

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
    pub(crate) game_time: GameTime,
    pub(crate) clocks: Option<WorldClocks>,
    pub(crate) ops: Option<OpList>,
    pub(crate) registries: Option<Versioned<RegistrySnapshot>>,
    pub(crate) dimensions: Option<RunningDimensions>,
    /// The players of the other worlds, in a tick whose input carries commands.
    pub(crate) players: Option<Vec<RosterRow>>,
    pub(crate) control: Vec<ToDim>,
    pub(crate) serverbound: Vec<InboundPlayerPacket>,
    /// The range that the coordinator grants for the request of the last tick.
    pub(crate) entity_id_grant: Option<EntityIdRange>,
}

/// What a dimension world hands the host after one tick.
#[derive(Default)]
pub struct DimOutput {
    /// The packets of the tick, in the order the dimension wrote them, each to one or more
    /// clients. The host stamps their `session` and `epoch`.
    pub(crate) packets: Vec<OutboundPlayerPacket>,
    /// The sessions whose players the dimension saved and let go after a despawn.
    pub(crate) released: Vec<PlayerSession>,
    pub(crate) attached: Vec<OutboundPlayerAttached>,
    pub(crate) traces: Vec<TraceEvent>,
    /// The players that left the dimension in the tick, after the packets of the tick.
    pub(crate) departures: Vec<Departure>,
    /// The changes of the players of the tick, for the muxer. A player that departs in the tick
    /// despawns in them.
    pub(crate) tiles: TileDeltas<PlayerData>,
    /// Present when the dimension holds few free entity ids.
    pub(crate) entity_id_request: Option<EntityIdRequest>,
    /// The players of the dimension at the end of the tick.
    pub(crate) roster: Vec<RosterRow>,
    /// The orders of the tick for players that other dimensions hold.
    pub(crate) orders: Vec<OrderedPlayer>,
    /// The changes to the world clocks that the tick made.
    pub(crate) clock_edits: Vec<EditClock>,
    /// The command runs of the tick that go on in other dimensions.
    pub(crate) handoffs: Vec<CommandHandoff>,
}

/// The messages of the input of this tick, until the inbox drain writes them at the sync point.
#[derive(Resource, Default)]
pub(crate) struct DimInbox {
    control: Vec<ToDim>,
    serverbound: Vec<InboundPlayerPacket>,
}

pub(crate) fn dim_world(sub_app: SubApp) -> DimWorld {
    SubAppWorld::new(sub_app, apply_input, take_output)
}

fn apply_input(world: &mut World, _tick: Tick, input: DimInput) {
    if let Some(mut time) = world.get_resource_mut::<GameTime>() {
        time.set_if_neq(input.game_time);
    }
    if let Some(clocks) = input.clocks {
        world.insert_resource(clocks);
    }
    if let Some(ops) = input.ops {
        world.insert_resource(ops);
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
}

fn take_output(world: &mut World) -> DimOutput {
    let mut output = DimOutput::default();
    // After every schedule of the tick, so the player is in no system's view once it leaves.
    player_dimension_change::depart_players(world, &mut output.departures);
    if let Some(mut packets) = world.get_resource_mut::<Messages<OutboundPlayerPacket>>() {
        output.packets.extend(packets.drain());
    }
    if let Some(mut released) = world.get_resource_mut::<Messages<OutboundPlayerReleased>>() {
        output
            .released
            .extend(released.drain().map(|released| released.session));
    }
    if let Some(mut attached) = world.get_resource_mut::<Messages<OutboundPlayerAttached>>() {
        output.attached.extend(attached.drain());
    }
    if let Some(log) = world.get_resource::<ColumnTraceLog>() {
        output.traces.extend(log.drain());
    }
    if let Some(mut events) = world.get_resource_mut::<TileEvents>() {
        output.tiles = events.take();
    }
    output.entity_id_request = world
        .get_resource::<CellEntityIds>()
        .and_then(CellEntityIds::request);
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
        output.orders = std::mem::take(&mut mail.orders);
        output.clock_edits = std::mem::take(&mut mail.clock_edits);
        output.handoffs = std::mem::take(&mut mail.handoffs);
    }
    output
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
    !inbox.control.is_empty() || !inbox.serverbound.is_empty()
}
