use crate::world_coordinate::WorldCoordinate;
use glam::{BVec3, DVec3, Vec3};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_numeric::{cos_nearest, radians_from_degrees, sin_nearest};

/// A position as a command writes it, resolved against a source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinates {
    /// The x, y and z axes, each absolute or relative to the position of the source.
    World([WorldCoordinate; 3]),
    /// Left, up and forwards along the axes of the source rotation.
    Local([f64; 3]),
}

pub(crate) fn local_position(
    anchor: DVec3,
    yaw: f32,
    pitch: f32,
    [left, up, forwards]: [f64; 3],
) -> DVec3 {
    let (cos_yaw, sin_yaw) = (
        cos_nearest(radians_from_degrees(yaw + 90.0)),
        sin_nearest(radians_from_degrees(yaw + 90.0)),
    );
    let (cos_pitch, sin_pitch) = (
        cos_nearest(radians_from_degrees(-pitch)),
        sin_nearest(radians_from_degrees(-pitch)),
    );
    let (cos_up_pitch, sin_up_pitch) = (
        cos_nearest(radians_from_degrees(-pitch + 90.0)),
        sin_nearest(radians_from_degrees(-pitch + 90.0)),
    );
    let forward = Vec3::new(cos_yaw * cos_pitch, sin_pitch, sin_yaw * cos_pitch).as_dvec3();
    let upward = Vec3::new(cos_yaw * cos_up_pitch, sin_up_pitch, sin_yaw * cos_up_pitch).as_dvec3();
    let leftward = -forward.cross(upward);
    anchor + forward * forwards + upward * up + leftward * left
}

impl Coordinates {
    /// The point the coordinates name against `source`.
    #[must_use]
    pub fn position<L>(&self, source: &Source<'_, L>) -> DVec3 {
        match self {
            Coordinates::World([x, y, z]) => DVec3::new(
                x.resolve(source.position.x),
                y.resolve(source.position.y),
                z.resolve(source.position.z),
            ),
            Coordinates::Local(local) => {
                local_position(source.anchor_position(), source.yaw, source.pitch, *local)
            }
        }
    }

    /// The axes that are relative to the source.
    #[must_use]
    pub fn relative(&self) -> BVec3 {
        match self {
            Coordinates::World(axes) => BVec3::from(axes.map(|axis| axis.relative)),
            Coordinates::Local(_) => BVec3::TRUE,
        }
    }
}
