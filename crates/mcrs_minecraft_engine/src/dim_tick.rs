use bevy_app::SubApp;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs::world::World;
use mcrs_engine_cell::{SubAppWorld, Tick};
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_engine_registries::RegistrySnapshot;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnTraceLog, TraceEvent};
use mcrs_minecraft_environment::world_clock::WorldClocks;

use crate::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerPacket, OutboundPlayerReleased, PlayerLatency,
};
use crate::channel_types::ToDim;
use crate::clock::GameTime;
use crate::ops::OpList;

/// A dimension world as the host ticks it.
pub type DimWorld = SubAppWorld<DimInput, DimOutput>;

/// What the host hands a dimension world for one tick. A copy of a host resource comes only
/// when the host changed it since the last tick; a new world gets every copy when it spawns.
pub struct DimInput {
    pub(crate) game_time: GameTime,
    pub(crate) clocks: Option<WorldClocks>,
    pub(crate) ops: Option<OpList>,
    pub(crate) registries: Option<Versioned<RegistrySnapshot>>,
    pub(crate) control: Vec<ToDim>,
    pub(crate) serverbound: Vec<InboundPlayerPacket>,
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
    /// The dimension changes that the tick asked for and that wait in the world.
    pub(crate) dimension_changes: usize,
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
    if let Some(mut inbox) = world.get_resource_mut::<DimInbox>() {
        inbox.control.extend(input.control);
        inbox.serverbound.extend(input.serverbound);
    }
}

fn take_output(world: &mut World) -> DimOutput {
    let mut output = DimOutput::default();
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
    output.dimension_changes = world
        .get_resource::<Messages<ChangeDimensionRequest>>()
        .map_or(0, Messages::len);
    output
}

/// Writes the messages of the input of this tick. The control messages go first, so a spawn
/// always precedes a serverbound packet of the same tick for the player it spawns.
pub(crate) fn drain_dim_inbox(
    mut inbox: ResMut<DimInbox>,
    mut spawns: ResMut<Messages<InboundPlayerSpawn>>,
    mut despawns: ResMut<Messages<InboundPlayerDespawn>>,
    mut serverbound: ResMut<Messages<InboundPlayerPacket>>,
    mut latencies: ResMut<Messages<PlayerLatency>>,
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
            ToDim::Latency(latency) => {
                latencies.write(latency);
            }
            ToDim::Serverbound(packet) => {
                serverbound.write(packet);
            }
        }
    }
    serverbound.write_batch(inbox.serverbound.drain(..));
}

pub(crate) fn inbox_holds_messages(inbox: Res<DimInbox>) -> bool {
    !inbox.control.is_empty() || !inbox.serverbound.is_empty()
}
