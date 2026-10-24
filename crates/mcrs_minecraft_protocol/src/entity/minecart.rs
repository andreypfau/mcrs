use crate::ByteAngle;
use glam::DVec3;
use mcrs_minecraft_wire::{Decode, Encode};

#[derive(Debug, Clone, Copy, Default, PartialEq, Encode, Decode)]
pub struct MinecartStep {
    pub position: DVec3,
    pub movement: DVec3,
    pub y_rot: ByteAngle,
    pub x_rot: ByteAngle,
    pub weight: f32,
}
