use crate::{Accept, Move, VehicleMove};

/// The fields of a serverbound packet that the checks read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Serverbound {
    Move(Move),
    Accept(Accept),
    VehicleMove(VehicleMove),
    ClientTickEnd,
    PlayerLoaded,
    /// The answer to a `ping`, with its id.
    Pong(i32),
    /// A packet whose fields no check of these layers reads.
    Other,
}
