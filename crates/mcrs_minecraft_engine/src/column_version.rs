use bevy_ecs::message::Message;
use bytes::Bytes;
use mcrs_voxel::ColumnPos;

#[derive(Message, Clone, Debug)]
pub struct ColumnVersion {
    pub column: ColumnPos,
    pub packet: Bytes,
}
