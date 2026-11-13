use alloc::boxed::Box;
use alloc::vec::Vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Plan;
use mcrs_minecraft_inventory_plan::Planned;

use crate::SlotRef;
use crate::commit::Settlement;

/// The buffers of the commit of a request, used again by each request.
#[derive(Resource, Default)]
pub(crate) struct RequestScratch {
    pub(crate) ops: Vec<Op<SlotRef, Planned<Entity>>>,
    pub(crate) plan: Box<Plan<SlotRef, Entity>>,
    pub(crate) settlement: Settlement,
}
