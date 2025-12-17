use bevy_math::DVec3;
use mcrs_protocol::ByteAngle;
use mcrs_protocol_macros::{Decode, Encode};

#[derive(Debug, Clone, Copy, Default, PartialEq, Encode, Decode)]
pub struct MinecartStep {
    pub position: DVec3,
    pub movement: DVec3,
    pub y_rot: ByteAngle,
    pub x_rot: ByteAngle,
    pub weight: f32,
}
