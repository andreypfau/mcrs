/// Where an icon button draws its icon and whether it draws its caption (WID-ICON-001).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconForm {
    /// The icon in the centre, no caption.
    IconOnly,
    /// The caption, and the icon at the right edge.
    IconRight,
    /// The caption, and the icon just after it when it fits.
    IconAfterText,
}
