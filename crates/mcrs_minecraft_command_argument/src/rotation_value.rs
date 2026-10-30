use crate::world_coordinate::WorldCoordinate;
use mcrs_minecraft_command::Source;
use mcrs_minecraft_core::Look;

/// A rotation as a command writes it, resolved against a source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationValue {
    /// The yaw in degrees.
    pub yaw: WorldCoordinate,
    /// The pitch in degrees.
    pub pitch: WorldCoordinate,
}

impl RotationValue {
    /// The yaw and the pitch against the source.
    #[must_use]
    pub fn resolve<L>(&self, source: &Source<'_, L>) -> Look {
        Look {
            yaw: self.yaw.resolve(f64::from(source.yaw)) as f32,
            pitch: self.pitch.resolve(f64::from(source.pitch)) as f32,
        }
    }
}
