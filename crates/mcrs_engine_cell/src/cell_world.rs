use crate::tick::Tick;

pub trait CellWorld: Send + 'static {
    type Input: Send + 'static;
    type Output: Send + 'static;

    fn tick(&mut self, tick: Tick, input: Self::Input) -> Self::Output;
}
