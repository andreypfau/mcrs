use mcrs_minecraft_wire::{Decode, Encode};

/// The yaw and pitch of an entity, in degrees.
#[derive(Copy, Clone, PartialEq, Default, Debug, Encode, Decode)]
pub struct Look {
    /// The yaw angle in degrees, where:
    /// - `-90` is looking east (towards positive x).
    /// - `0` is looking south (towards positive z).
    /// - `90` is looking west (towards negative x).
    /// - `180` is looking north (towards negative z).
    ///
    /// Values -180 to 180 are also valid.
    pub yaw: f32,
    /// The pitch angle in degrees, where:
    /// - `-90` is looking straight up.
    /// - `0` is looking straight ahead.
    /// - `90` is looking straight down.
    pub pitch: f32,
}
