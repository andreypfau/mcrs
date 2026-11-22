use crate::EntityUuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cause {
    pub causing: Option<EntityUuid>,
    pub direct: Option<EntityUuid>,
}
