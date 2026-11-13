/// What a finish action does to the block of a dig.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// The block breaks now and the dig ends.
    Break,
    /// The block stays. The dig goes on as a delayed break, which breaks the block at a progress
    /// of 1 or more.
    DelayedBreak,
    /// The block stays and no delayed break starts: with a rate of 0 it would never end.
    Stay,
}
