use crate::{DrawKind, Group};

pub(crate) struct PendingGroup {
    pub(crate) kind: DrawKind,
    pub(crate) group: Group,
}
