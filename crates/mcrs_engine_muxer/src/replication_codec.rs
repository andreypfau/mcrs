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
/// A viewer never gets a spawn or a despawn from a [`TileBlob`]: the muxer calls [`Self::hold`]
/// for each viewer that gains or loses a subject, so the codec knows what each client holds.
pub trait ReplicationCodec {
    type Data;
    /// A message of the game to the codec, such as a value that the host measures.
    type Input;

    /// Takes in the events of one tile delta of tick `tick`. A subject that enters the tile gets
    /// its absolute state in its next [`BlobKind::Delta`] blob, because the viewers of the tile
    /// may hold it from its old tile at another tier.
    fn apply(&mut self, tick: u64, delta: &TileDelta<Self::Data>);

    /// The state of `subjects`, sorted, at `tier`.
    ///
    /// A [`BlobKind::Delta`] blob holds the moves from the bases to the latest state at tick
    /// `tick`, and moves the bases there. All viewers of the tier share it. A
    /// [`BlobKind::Keyframe`] blob holds the absolute state at the bases and moves no base. A
    /// viewer that changes to `tier` receives it.
    fn blob(&mut self, tick: u64, tier: Tier, subjects: &[EntityId], kind: BlobKind) -> TileBlob;

    /// Makes the next [`BlobKind::Delta`] blob of each tier send the absolute state of `subject`,
    /// for a viewer whose client holds the subject at another tier. The muxer calls it when a
    /// subject changes layer, because only the muxer knows the layer ranks.
    fn refresh(&mut self, subject: EntityId);

    /// Applies a change of what the client of `viewer` holds, and appends the bytes for the
    /// client to `chunks`.
    fn hold(&mut self, viewer: ViewerId, change: Hold<'_>, chunks: &mut Vec<Bytes>);

    /// Ends tick `tick`, after its tile deltas, its spawns and its removals. Appends the bytes
    /// that the codec owes to viewers, also to a viewer that subscribes no more.
    fn finish_tick(&mut self, tick: u64, out: &mut Vec<ViewerBytes>);

    fn receive(&mut self, input: Self::Input);

    /// The latest position of `subject` as `[x, y, z]`, in the units of the
    /// tile width of the [`Muxer`](crate::Muxer). The muxer measures the
    /// distance of the tiles to a viewer from the position of its own subject.
    fn position(&self, subject: EntityId) -> Option<[f64; 3]>;
}

/// The kind of a [`TileBlob`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobKind {
    Delta,
    Keyframe,
}

/// A change of what the client of a viewer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold<'a> {
    /// The client gains `subject` at its base at `tier`. The codec appends the bytes that make
    /// the subject appear.
    Gain { subject: EntityId, tier: Tier },
    /// The client loses `subjects`. The codec appends the bytes that remove them.
    Lose { subjects: &'a [EntityId] },
    /// The viewer no longer subscribes. The codec drops the bytes it owes the viewer, owes it no
    /// more, and appends nothing.
    Forget,
}
