use mcrs_engine_muxer::ViewerId;

/// A message of the host to a [`PlayerCodec`](crate::PlayerCodec): the round trip of `viewer` in
/// milliseconds, which the host measures on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerInput {
    pub viewer: ViewerId,
    pub latency: i32,
}
