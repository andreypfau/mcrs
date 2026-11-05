use bevy_ecs::message::Message;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_voxel::ColumnPos;

use crate::bus::{PacketPayload, PacketPriority};

/// A change of a column that the muxer subscribes to, for the clients that hold the column.
#[derive(Message, Clone, Debug)]
pub struct ColumnChange {
    pub column: ColumnPos,
    pub priority: PacketPriority,
    pub payload: PacketPayload,
    /// A holder that does not get the change, because its client made the change itself.
    pub except: Option<PlayerSession>,
}
