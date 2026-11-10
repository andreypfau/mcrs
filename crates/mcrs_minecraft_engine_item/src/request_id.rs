/// The number that the sender of a request selects. The result of the request repeats it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestId(pub u32);
