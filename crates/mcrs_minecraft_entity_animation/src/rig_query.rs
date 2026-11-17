use mcrs_molang::QuerySlot;

use crate::entity_query::EntityQuery;
use crate::rig::Rig;

impl Rig {
    #[must_use]
    pub fn slot_query(&self, slot: QuerySlot) -> Option<EntityQuery> {
        let bindings = &self.queries;
        if bindings.delta_time == Some(slot) {
            return Some(EntityQuery::DeltaTime);
        }
        if bindings.anim_time == Some(slot) {
            return Some(EntityQuery::AnimTime);
        }
        if bindings.key_frame_lerp_time == Some(slot) {
            return Some(EntityQuery::KeyFrameLerpTime);
        }
        bindings
            .host
            .iter()
            .find(|host| host.slot == slot)
            .map(|host| host.query)
    }
}
