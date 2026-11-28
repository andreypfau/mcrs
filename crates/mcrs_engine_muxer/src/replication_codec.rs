use bytes::Bytes;

use crate::entity_id::ViewerId;
use crate::tier::Tier;
use crate::tile_blob::TileBlob;
use crate::tile_delta::TileDelta;
use crate::viewer_bytes::ViewerBytes;
use mcrs_minecraft_entity_key::EntityId;

pub trait ReplicationCodec {
    type Data;
    type Input;

    fn apply(&mut self, tick: u64, delta: &TileDelta<Self::Data>);

    fn blob(&mut self, tick: u64, tier: Tier, subjects: &[EntityId], kind: BlobKind) -> TileBlob;

    fn refresh(&mut self, subject: EntityId);

    fn hold(&mut self, viewer: ViewerId, change: Hold<'_>, chunks: &mut Vec<Bytes>);

    fn finish_tick(&mut self, tick: u64, out: &mut Vec<ViewerBytes>);

    fn receive(&mut self, input: Self::Input);

    fn tracking_range(&self, subject: EntityId) -> Option<u32>;

    fn position(&self, subject: EntityId) -> Option<[f64; 3]>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobKind {
    Delta,
    Keyframe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold<'a> {
    Gain { subject: EntityId, tier: Tier },
    Lose { subjects: &'a [EntityId] },
    Forget,
}
