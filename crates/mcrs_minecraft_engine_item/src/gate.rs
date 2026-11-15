use bevy_ecs::world::World;
use mcrs_minecraft_inventory_transaction::Filter;

use crate::GateId;
use crate::GateQuery;

/// The answer of the take filter or of the place filter of a slot: a constant, or a question to a
/// function that [`register_slot_gate`](crate::ItemAppExt::register_slot_gate) added. A constant
/// costs no function call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gate {
    Permits,
    Refuses,
    /// Asks the function with this id. An id that the world does not have refuses.
    Ask(GateId),
}

/// A gate function. It reads the world and answers for the slot, the actor and the stack of the
/// query, and gives the same answer for each operation of one request (ITX-LIM-006).
pub type GateFn = fn(&World, &GateQuery) -> Filter;
