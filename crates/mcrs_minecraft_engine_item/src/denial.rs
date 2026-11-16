use crate::RequestId;

/// Who denied a request and a code that this source selects. The edge renders it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Denial {
    /// The id that names the sender of the denial.
    pub source: RequestId,
    /// The cause, in the numbers of that sender.
    pub code: u16,
}
