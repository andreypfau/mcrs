use bevy_ecs::message::Message;
use mcrs_voxel::ColumnPos;

use crate::column_packet::StoredLight;
use crate::stored_column::StoredColumn;

/// A version of a column that a dimension publishes for the muxer: the dimension took it from
/// its sections, or the data layer made it. It holds every change of the ticks before the tick
/// that publishes it. The muxer applies each change of that tick and of the later ticks after it.
#[derive(Message, Clone, Debug)]
pub struct ColumnVersion {
    pub(crate) column: ColumnPos,
    pub(crate) stored: StoredColumn,
    pub(crate) light: StoredLight,
}
