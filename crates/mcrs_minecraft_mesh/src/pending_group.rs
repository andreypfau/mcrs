use crate::{DrawKind, Group};

/// A group the sink holds until section assembly sorts it into the bucket of its kind.
pub(crate) struct PendingGroup {
    pub(crate) kind: DrawKind,
    pub(crate) group: Group,
}
