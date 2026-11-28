use bevy_ecs::world::World;
use mcrs_minecraft_inventory_transaction::Filter;

use crate::GateId;
use crate::GateQuery;
use crate::slot_gates::SlotGates;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gate {
    Permits,
    Refuses,
    Ask(GateId),
}

pub type GateFn = fn(&World, &GateQuery) -> Filter;

impl Gate {
    #[must_use]
    pub fn answer(self, world: &World, query: &GateQuery) -> Filter {
        match self {
            Self::Permits => Filter::Permits,
            Self::Refuses => Filter::Refuses,
            Self::Ask(id) => {
                let function = world
                    .get_resource::<SlotGates>()
                    .and_then(|gates| gates.get(id));
                function.map_or(Filter::Refuses, |function| function(world, query))
            }
        }
    }
}
