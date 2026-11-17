use crate::{Accept, Move, VehicleMove};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Serverbound {
    Move(Move),
    Accept(Accept),
    VehicleMove(VehicleMove),
    ClientTickEnd,
    PlayerLoaded,
    Pong(i32),
    Other,
}
