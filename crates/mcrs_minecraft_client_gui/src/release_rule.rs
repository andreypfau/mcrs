use crate::open_drag::OpenDrag;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReleaseRule {
    #[default]
    NoClick,
    Click,
    Drag(OpenDrag),
}
