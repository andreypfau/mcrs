use std::ops::Range;

use bytes::Bytes;

use crate::entity_id::EntityId;

/// The bytes of one tile delta, which every viewer of the tile shares.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TileBlob {
    pub bytes: Bytes,
    /// The bytes of each subject with events, in ascending order of offset. The muxer cuts a
    /// range out for a viewer that must not get those events, such as the viewer's own subject.
    pub ranges: Vec<SubjectRange>,
}

/// The bytes of the events of one subject inside a [`TileBlob`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectRange {
    pub subject: EntityId,
    pub range: Range<usize>,
}
