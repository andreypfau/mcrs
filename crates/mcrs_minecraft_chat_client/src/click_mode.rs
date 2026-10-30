/// The modifier state of a click on a chat line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickMode {
    Plain,
    Shift,
}
