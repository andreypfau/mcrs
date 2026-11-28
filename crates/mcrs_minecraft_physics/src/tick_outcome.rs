#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TickOutcome {
    pub landed: bool,
    pub stuck_in_block: bool,
    pub bounced: bool,
}
