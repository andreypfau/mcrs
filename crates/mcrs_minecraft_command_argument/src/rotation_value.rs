use crate::world_coordinate::WorldCoordinate;
use mcrs_minecraft_command::Source;
use mcrs_minecraft_core::Look;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationValue {
    pub yaw: WorldCoordinate,
    pub pitch: WorldCoordinate,
}

impl RotationValue {
    #[must_use]
    pub fn resolve<L>(&self, source: &Source<'_, L>) -> Look {
        Look {
            yaw: self.yaw.resolve(f64::from(source.yaw)) as f32,
            pitch: self.pitch.resolve(f64::from(source.pitch)) as f32,
        }
    }
}
