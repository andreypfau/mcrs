use crate::tick::Tick;

/// A world that ticks: one owned input goes in, one owned output comes out.
///
/// A world owns all of its state and reads no clock, so its outputs depend only on its inputs.
/// [`Cells`](crate::Cells) calls it from one thread at a time, on the thread that its
/// [`Placement`](crate::Placement) picks.
pub trait CellWorld: Send + 'static {
    type Input: Send + 'static;
    type Output: Send + 'static;

    fn tick(&mut self, tick: Tick, input: Self::Input) -> Self::Output;
}
