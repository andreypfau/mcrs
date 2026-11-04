use bytes::Bytes;

use crate::entity_id::ViewerId;

/// The bytes for one viewer, in the order to append them to its connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerBytes {
    pub viewer: ViewerId,
    pub chunks: Vec<Bytes>,
}
