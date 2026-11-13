/// The shape of the mouse pointer in a frame (WID-STATE-004).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerShape {
    #[default]
    Normal,
    Hand,
    ResizeHorizontal,
    ResizeVertical,
    NotAllowed,
    /// The text beam over an editable edit box (EDIT-PTR-004).
    Text,
}
