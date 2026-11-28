use bevy_ecs::message::Message;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_voxel::ColumnPos;

use crate::bus::PacketPayload;

#[derive(Message, Clone, Debug)]
pub struct ColumnChange {
    pub column: ColumnPos,
    pub payload: PacketPayload,
    pub except: Option<PlayerSession>,
}
