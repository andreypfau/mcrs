/// The serverbound packets a dimension takes in one tick, from every player of the dimension
/// together. A vanilla client sends three or more a tick, so a few hundred would cap a dimension
/// at some seventy players. A packet that finds the input of the tick full waits in its
/// session's `PendingInbound`.
pub const TO_DIM_CAPACITY: usize = 4096;

/// The serverbound packets one session may hold for its dimension, behind a full input, before
/// the host disconnects it. The bound is per session, so a dimension that falls behind costs
/// each player only its own backlog, never another player's.
pub const MAX_HELD_SERVERBOUND: usize = 256;
