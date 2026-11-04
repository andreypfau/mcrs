/// A logical tick number. All worlds of one [`Cells`](crate::Cells) tick the same `Tick`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tick(pub u64);
