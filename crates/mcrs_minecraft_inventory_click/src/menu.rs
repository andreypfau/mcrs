/// The menu that a click goes to: its entries and the rules that the specification of each menu
/// gives.
pub trait Menu {
    type Slot;

    /// The number of entries.
    fn entry_count(&self) -> u16;

    /// The slot that the entry `index` refers to, or `None` when the menu has no such entry.
    fn slot(&self, index: u16) -> Option<Self::Slot>;

    /// The destinations of a quick move from the entry `source`, in order, for the slots as they
    /// are now.
    fn quick_move_destinations(&self, source: u16) -> impl Iterator<Item = u16>;

    /// Whether pick up all can take from the entry `index`.
    fn permits_pick_up_all(&self, index: u16) -> bool;
}
