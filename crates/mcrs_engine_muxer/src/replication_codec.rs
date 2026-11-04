use bytes::Bytes;

use crate::entity_id::EntityId;
use crate::tile_blob::TileBlob;
use crate::tile_delta::TileDelta;

/// The encoding of entity events into the bytes of the game's protocol.
///
/// The muxer gives the codec each tile delta exactly once and in tick order, also when the tile
/// has no viewer. Thus the codec can keep the latest state of each subject, such as a delta base,
/// and build a spawn bundle from it at any time.
pub trait ReplicationCodec {
    type Data;

    /// Encodes the events of one tile delta. All viewers of the tile share the result.
    fn encode(&mut self, delta: &TileDelta<Self::Data>) -> TileBlob;

    /// The bytes that make `subject` appear on a client, from its latest state.
    fn spawn_bundle(&mut self, subject: EntityId) -> Bytes;

    /// The bytes that remove `subjects` from a client.
    fn remove(&mut self, subjects: &[EntityId]) -> Bytes;
}
