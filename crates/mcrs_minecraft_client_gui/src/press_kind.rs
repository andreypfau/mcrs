#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PressKind {
    #[default]
    Single,
    Double,
}
