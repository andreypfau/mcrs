pub trait Menu {
    type Slot;

    fn entry_count(&self) -> u16;

    fn slot(&self, index: u16) -> Option<Self::Slot>;

    fn quick_move_destinations(
        &self,
        source: u16,
        holds_empty: impl Fn(u16) -> bool,
    ) -> impl Iterator<Item = u16>;

    fn permits_pick_up_all(&self, index: u16) -> bool;

    fn permits_drag(&self, index: u16) -> bool;
}
