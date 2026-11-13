/// The state of a tab that picks its sprite (TAB-MENU-002).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabSprite {
    /// The tab is the current tab of its bar.
    pub current: bool,
    pub highlighted: bool,
}
