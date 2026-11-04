/// Where the far end of a [`Link`](crate::Link) runs.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    /// On the thread of the near end, when the near end takes in the outputs. The stage then
    /// runs until it owes nothing, so the outputs do not depend on thread timing.
    Inline,
    /// On an OS thread of its own, which runs whenever an input or a task outcome arrives.
    Thread,
}
