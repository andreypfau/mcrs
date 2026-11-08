use mcrs_engine_muxer::ViewerId;

/// A message of the host to a [`PlayerCodec`](crate::PlayerCodec).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerInput {
    /// The round trip of `viewer` in milliseconds, which the host measures on the wire.
    Latency { viewer: ViewerId, latency: i32 },
    /// Whether the connection of `viewer` holds more bytes than its client reads.
    Backlog { viewer: ViewerId, backlog: Backlog },
}

/// The state of the bytes that wait on the connection of a viewer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backlog {
    /// The client reads what the server sends.
    #[default]
    Clear,
    /// The client reads less than the server sends. The codec holds back the moves of the mobs
    /// and later sends the latest state of each one instead of every step.
    Behind,
}
