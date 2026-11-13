use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Planned;

/// A slot as the kernel read it before the first operation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Seen<I, T> {
    pub(crate) slot: I,
    pub(crate) stack: Option<Planned<T>>,
    pub(crate) limit: u8,
    pub(crate) take: Filter,
}

impl<I, T> Seen<I, T> {
    /// The capacity of this slot for `stack`.
    pub(crate) fn cap(&self, stack: Planned<T>) -> u8 {
        self.limit.min(stack.max)
    }

    pub(crate) fn takes(&self) -> bool {
        self.take == Filter::Permits
    }
}
