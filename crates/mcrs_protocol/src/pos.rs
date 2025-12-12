use crate::{Decode, Encode};
use bevy_ecs::component::Component;
use bevy_math::{DVec3, Quat};
use bitfield_struct::bitfield;
use derive_more::Deref;

#[derive(Component, Clone, Copy, PartialEq, Debug, Default, Deref)]
pub struct Position(DVec3);

impl Position {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }
}

impl From<DVec3> for Position {
    fn from(value: DVec3) -> Self {
        Position(value)
    }
}

impl From<Position> for DVec3 {
    fn from(value: Position) -> Self {
        value.0
    }
}

impl Encode for Position {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        let pos = self.0;
        pos.encode(&mut w)?;
        Ok(())
    }
}

impl Decode<'_> for Position {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let pos = DVec3::decode(r)?;
        Ok(Position(pos))
    }
}

#[derive(Component, Copy, Clone, PartialEq, Default, Debug)]
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

impl From<Look> for Quat {
    fn from(value: Look) -> Self {
        Quat::from_euler(
            bevy_math::EulerRot::YXZ,
            value.yaw.to_radians(),
            value.pitch.to_radians(),
            0.0,
        )
    }
}

impl Encode for Look {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        self.yaw.encode(&mut w)?;
        self.pitch.encode(&mut w)?;
        Ok(())
    }
}

impl Decode<'_> for Look {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let yaw = f32::decode(r)?;
        let pitch = f32::decode(r)?;
        Ok(Look { yaw, pitch })
    }
}

#[bitfield(u8)]
pub struct MoveFlags {
    pub on_ground: bool,
    pub horizontal_collision: bool,
    #[bits(6)]
    _pad: u8,
}

impl Encode for MoveFlags {
    fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
        self.0.encode(&mut w)?;
        Ok(())
    }
}

impl Decode<'_> for MoveFlags {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let byte = u8::decode(r)?;
        Ok(MoveFlags::from_bits(byte))
    }
}
