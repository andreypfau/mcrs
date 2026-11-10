use alloc::boxed::Box;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_inventory_plan::Plan;

use crate::SlotRef;
use crate::commit::Settlement;

/// The buffers of the commit of a request, used again by each request.
#[derive(Resource)]
pub(crate) struct RequestScratch {
    pub(crate) plan: Box<Plan<SlotRef, Entity>>,
    pub(crate) settlement: Settlement,
}

impl Default for RequestScratch {
    fn default() -> Self {
        Self {
            plan: Box::new(Plan::new()),
            settlement: Settlement::default(),
        }
    }
}
