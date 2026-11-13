/// The delay of the tooltip of a widget; the screen keeps the tooltip text (WID-TIP-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TooltipDelay {
    /// The time in milliseconds that the widget wants its tooltip before the tooltip shows.
    pub delay_ms: u32,
}
