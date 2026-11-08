use crate::Disconnect;

/// What layer 1 does with one serverbound packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Forward,
    /// Forward the `client_tick_end` that ends this client tick, numbered from 1.
    EndTick(u64),
    /// Do not hand the packet on: an answer to a probe, or a packet the server ignores.
    Drop,
    Disconnect(Disconnect),
}
