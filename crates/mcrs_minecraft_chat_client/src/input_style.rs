/// How the initial text of the screen shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputStyle {
    #[default]
    Plain,
    /// The draft style, until the first change.
    Draft,
}
