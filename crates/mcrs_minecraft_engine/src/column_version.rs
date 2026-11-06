use bevy_ecs::message::Message;
use bytes::Bytes;
use mcrs_voxel::ColumnPos;

/// A version of a column that a dimension publishes for the muxer, as the `level_chunk_with_light`
/// packet with its id: the dimension took it from its sections, or the data layer made it. It
/// holds every change of the ticks before the tick that publishes it. The muxer applies each
/// change of that tick and of the later ticks after it.
#[derive(Message, Clone, Debug)]
pub struct ColumnVersion {
    pub column: ColumnPos,
    pub packet: Bytes,
}
