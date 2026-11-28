#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerShape {
    #[default]
    Normal,
    Hand,
    ResizeHorizontal,
    ResizeVertical,
    NotAllowed,
    Text,
}
