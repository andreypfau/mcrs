use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;

use crate::Aabb;
use crate::GridFlags;
use crate::PushTargets;
use crate::TeamSlot;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridRow {
    pub position: DVec3,
    pub previous: DVec3,
    pub half_width: f32,
    pub height: f32,
    pub key: EntityUuid,
    pub id: EntityId,
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
