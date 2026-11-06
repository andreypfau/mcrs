//! The messages that the coordinator sends the muxer.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_engine_muxer::TileDeltas;
use mcrs_minecraft_dimension::{Dimension as DimensionKey, DimensionType};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_replication::PlayerData;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

use crate::bus::{InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerPacket};
use crate::column_change::ColumnChange;
use crate::column_version::ColumnVersion;
use crate::command_handoff::CommandHandoff;
use crate::departure::Departure;
use crate::player_order::OrderedPlayer;
use crate::player_view::{PlayerView, ViewerPosition};

/// The dimension `dim` runs and takes input from now on.
#[derive(Message, Clone, Copy, Debug)]
pub struct DimOpened {
    pub dim: Entity,
}

/// The dimension `dim` has stopped. Its input, its muxer world and the players that wait for it
/// go.
#[derive(Message, Clone, Copy, Debug)]
pub struct DimClosed {
    pub dim: Entity,
}

/// What the dimension `dim` handed over in one tick for its clients. The coordinator writes one
/// for each dimension, in the order the dimensions ticked.
#[derive(Message)]
pub struct DimDelivered {
    pub dim: Entity,
    pub client: ClientOutput,
    /// The players that left the dimension in the tick, after the packets of the tick.
    pub departures: Vec<RoutedDeparture>,
}

/// What the cells of a dimension hand over in one tick for the muxer.
#[derive(Default)]
pub struct ClientOutput {
    /// The packets of the tick, in the order the dimension wrote them, each to one or more
    /// clients. The muxer stamps their `session` and `epoch`.
    pub packets: Vec<OutboundPlayerPacket>,
    /// The sessions whose players the dimension saved and let go after a despawn.
    pub released: Vec<PlayerSession>,
    pub attached: Vec<OutboundPlayerAttached>,
    /// The changes of the players of the tick, for the entity muxer. A player that departs in
    /// the tick despawns in them.
    pub tiles: TileDeltas<PlayerData>,
    /// The orders of the tick for players of other dimensions, for the dimension that holds
    /// each player once every departure of the tick is routed.
    pub orders: Vec<OrderedPlayer>,
    /// The command runs of the tick that go on in the dimension each one names.
    pub handoffs: Vec<CommandHandoff>,
    /// The views of the players that changed in the tick, in the order they changed.
    pub views: Vec<PlayerView>,
    /// The versions of the columns that the muxer wanted, published in the tick.
    pub column_versions: Vec<ColumnVersion>,
    /// The changes of the tick to the columns that the muxer subscribes to, after the versions.
    pub column_changes: Vec<ColumnChange>,
    /// Where the players that moved in the tick stand, before the changes.
    pub positions: Vec<ViewerPosition>,
}

impl ClientOutput {
    /// Appends what another cell of the same dimension handed over in the same tick. The tile
    /// deltas stay unsorted until [`TileDeltas::coalesce`].
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
    }
}

/// A player that left its dimension, with the dimension that the coordinator sends it to.
pub struct RoutedDeparture {
    pub departure: Departure,
    /// The running dimension that the departure names, unless it names its own. `None` sends
    /// the player back where it was.
    pub destination: Option<Destination>,
}

/// A running dimension that a player arrives in.
#[derive(Clone, Debug)]
pub struct Destination {
    pub label: Entity,
    pub key: ResourceKeyBuf<DimensionKey>,
    pub dimension_type: Id<DimensionType>,
}

/// The dimension where the joining player of `spawn.session` spawns.
#[derive(Message, Clone, Debug)]
pub struct JoinPlaced {
    pub dim: Entity,
    pub spawn: InboundPlayerSpawn,
}
