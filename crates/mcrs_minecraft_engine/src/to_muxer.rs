use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_engine_muxer::TileDeltas;
use mcrs_minecraft_dimension::{Dimension as DimensionKey, DimensionType};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_replication::ReplicationData;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

use crate::block_action_ack::BlockActionAck;
use crate::bus::{InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerPacket};
use crate::column_change::ColumnChange;
use crate::column_version::ColumnVersion;
use crate::command_handoff::CommandHandoff;
use crate::departure::Departure;
use crate::player_order::OrderedPlayer;
use crate::player_view::{PlayerView, ViewerPosition};

#[derive(Message, Clone, Copy, Debug)]
pub struct DimOpened {
    pub dim: Entity,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct DimClosed {
    pub dim: Entity,
}

#[derive(Message)]
pub struct DimDelivered {
    pub dim: Entity,
    pub client: ClientOutput,
    pub departures: Vec<RoutedDeparture>,
}

#[derive(Default)]
pub struct ClientOutput {
    pub packets: Vec<OutboundPlayerPacket>,
    pub released: Vec<PlayerSession>,
    pub attached: Vec<OutboundPlayerAttached>,
    pub tiles: TileDeltas<ReplicationData>,
    pub orders: Vec<OrderedPlayer>,
    pub handoffs: Vec<CommandHandoff>,
    pub views: Vec<PlayerView>,
    pub column_versions: Vec<ColumnVersion>,
    pub column_changes: Vec<ColumnChange>,
    pub positions: Vec<ViewerPosition>,
    pub block_acks: Vec<BlockActionAck>,
}

impl ClientOutput {
    pub(crate) fn append(&mut self, other: Self) {
        self.packets.extend(other.packets);
        self.released.extend(other.released);
        self.attached.extend(other.attached);
        self.tiles.tiles.extend(other.tiles.tiles);
        self.orders.extend(other.orders);
        self.handoffs.extend(other.handoffs);
        self.views.extend(other.views);
        self.column_versions.extend(other.column_versions);
        self.column_changes.extend(other.column_changes);
        self.positions.extend(other.positions);
        self.block_acks.extend(other.block_acks);
    }
}

pub struct RoutedDeparture {
    pub departure: Departure,
    pub destination: Option<Destination>,
}

#[derive(Clone, Debug)]
pub struct Destination {
    pub label: Entity,
    pub key: ResourceKeyBuf<DimensionKey>,
    pub dimension_type: Id<DimensionType>,
}

#[derive(Message, Clone, Debug)]
pub struct JoinPlaced {
    pub dim: Entity,
    pub spawn: InboundPlayerSpawn,
}
