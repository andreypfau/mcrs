use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;

use crate::Aabb;
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
    pub key: EntityKey,
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
        Aabb::of_body(self.position, self.half_width * 2.0, self.height)
    }
}
