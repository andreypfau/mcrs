use bytes::Bytes;

use crate::entity_id::{EntityId, ViewerId};
use crate::tier::Tier;
use crate::tile_blob::TileBlob;
use crate::tile_delta::TileDelta;
use crate::viewer_bytes::ViewerBytes;

/// The encoding of entity events into the bytes of the game's protocol.
///
/// The muxer gives the codec each tile delta exactly once and in tick order, also when the tile
/// has no viewer. Thus the codec can keep the latest state of each subject and build its bytes
/// from it at any time.
///
/// The codec keeps one delta base for each subject and [`Tier`]: what the viewers of that tier
/// last received of the subject. A client that holds a subject at a tier always shows the base
/// of that tier, so all viewers of a tier share the same bytes.
///
/// A viewer never gets a spawn or a despawn from a [`TileBlob`]: the muxer calls
/// [`Self::spawn`] and [`Self::remove`] for each viewer that gains or loses a subject, so the
/// codec knows what each client holds.
pub trait ReplicationCodec {
    type Data;
    /// A message of the game to the codec, such as a value that the host measures.
    type Input;

    /// Takes in the events of one tile delta of tick `tick`.
    fn apply(&mut self, tick: u64, delta: &TileDelta<Self::Data>);

    /// The moves of `subjects`, sorted, from their bases at `tier` to their latest state at tick
    /// `tick`, and moves the bases there. All viewers of the tier share the result.
    fn encode(&mut self, tick: u64, tier: Tier, subjects: &[EntityId]) -> TileBlob;

    /// The absolute state of `subjects`, sorted, at their bases at `tier`. It moves no base. A
    /// viewer that changes to `tier` receives it.
    fn keyframe(&mut self, tier: Tier, subjects: &[EntityId]) -> TileBlob;

    /// Makes the next [`Self::encode`] of each tier send the absolute state of `subject`, for a
    /// viewer whose client holds the subject at another tier.
    fn refresh(&mut self, subject: EntityId);

    /// Appends the bytes that make `subject` appear on the client of `viewer` at its base at
    /// `tier`.
    fn spawn(&mut self, viewer: ViewerId, subject: EntityId, tier: Tier, chunks: &mut Vec<Bytes>);

    /// The bytes that remove `subjects` from the client of `viewer`.
    fn remove(&mut self, viewer: ViewerId, subjects: &[EntityId]) -> Bytes;

    /// Ends tick `tick`, after its tile deltas, its spawns and its removals. Appends the bytes
    /// that the codec owes to viewers, also to a viewer that subscribes no more.
    fn finish_tick(&mut self, tick: u64, out: &mut Vec<ViewerBytes>);

    /// Forgets `viewer`, which no longer subscribes: the codec drops the bytes it owes the
    /// viewer and owes it no more.
    fn forget(&mut self, viewer: ViewerId);

    fn receive(&mut self, input: Self::Input);

    /// The latest position of `subject` as `[x, y, z]`, in the units of the
    /// [`MuxerSettings::tile_width`](crate::MuxerSettings::tile_width). The muxer measures the
    /// distance of the tiles to a viewer from the position of its own subject.
    fn position(&self, subject: EntityId) -> Option<[f64; 3]>;
}
