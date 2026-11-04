use mcrs_engine_muxer::ViewerId;

/// A message of the host to a [`PlayerCodec`](crate::PlayerCodec).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerInput {
    /// The round trip of `viewer` in milliseconds, which the host measures on the wire.
    Latency { viewer: ViewerId, latency: i32 },
}
