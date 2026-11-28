use bytes::Bytes;

use crate::entity_id::ViewerId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerBytes {
    pub viewer: ViewerId,
    pub chunks: Vec<Bytes>,
}
