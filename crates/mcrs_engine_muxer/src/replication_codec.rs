use bytes::Bytes;

use crate::entity_id::{EntityId, ViewerId};
use crate::tile_blob::TileBlob;
use crate::tile_delta::TileDelta;
use crate::viewer_bytes::ViewerBytes;

/// The encoding of entity events into the bytes of the game's protocol.
///
/// The muxer gives the codec each tile delta exactly once and in tick order, also when the tile
/// has no viewer. Thus the codec can keep the latest state of each subject, such as a delta base,
/// and build a spawn bundle from it at any time.
///
/// A viewer never gets a spawn or a despawn from a [`TileBlob`]: the muxer calls
/// [`Self::spawn`] and [`Self::remove`] for each viewer that gains or loses a subject, so the
/// codec knows what each client holds.
pub trait ReplicationCodec {
    type Data;
    /// A message of the game to the codec, such as a value that the host measures.
    type Input;

    /// Encodes the events of one tile delta of tick `tick`. All viewers of the tile share the
    /// result.
    fn encode(&mut self, tick: u64, delta: &TileDelta<Self::Data>) -> TileBlob;

    /// Appends the bytes that make `subject` appear on the client of `viewer`, from its latest
    /// state.
    fn spawn(&mut self, viewer: ViewerId, subject: EntityId, chunks: &mut Vec<Bytes>);

    /// The bytes that remove `subjects` from the client of `viewer`.
    fn remove(&mut self, viewer: ViewerId, subjects: &[EntityId]) -> Bytes;

    /// Ends tick `tick`, after its tile deltas, its spawns and its removals. Appends the bytes
    /// that the codec owes to viewers, also to a viewer that subscribes no more.
    fn finish_tick(&mut self, tick: u64, out: &mut Vec<ViewerBytes>);

    fn receive(&mut self, input: Self::Input);
}
