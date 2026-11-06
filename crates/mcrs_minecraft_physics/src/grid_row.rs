use glam::DVec3;
use mcrs_engine_muxer::EntityId;

use crate::Aabb;
use crate::BirthKey;
use crate::GridFlags;
use crate::PushTargets;
use crate::TeamSlot;

/// The values of a body in the grid that the push of another entity reads, from the start of
/// the tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridRow {
    pub position: DVec3,
    /// `f32(width / 2)`.
    pub half_width: f32,
    pub height: f32,
    pub key: BirthKey,
    pub id: EntityId,
    /// The root of the vehicle tree of the entity; its own id when it rides nothing.
    pub root_vehicle: EntityId,
    pub team: TeamSlot,
    pub flags: GridFlags,
    pub push_targets: PushTargets,
}

impl GridRow {
    #[must_use]
    pub fn bounds(&self) -> Aabb {
        let half_width = f64::from(self.half_width);
        Aabb::new(
            DVec3::new(
                self.position.x - half_width,
                self.position.y,
                self.position.z - half_width,
            ),
            DVec3::new(
                self.position.x + half_width,
                self.position.y + f64::from(self.height),
                self.position.z + half_width,
            ),
        )
    }
}
