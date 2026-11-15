use alloc::vec::Vec;

use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory_transaction::Filter;

use crate::Gate;
use crate::GateFn;
use crate::GateId;
use crate::GateQuery;
use crate::RegisterError;

/// The gate functions of a world, in the order of their registration.
#[derive(Resource, Default)]
pub(crate) struct SlotGates {
    gates: Vec<GateFn>,
}

impl SlotGates {
    pub(crate) fn register(&mut self, gate: GateFn) -> Result<GateId, RegisterError> {
        let index = u16::try_from(self.gates.len())
            .ok()
            .filter(|&index| index < u16::MAX)
            .ok_or(RegisterError::GateTableFull)?;
        self.gates.push(gate);
        Ok(GateId(index))
    }
}

pub(crate) fn answer(gate: Gate, world: &World, query: &GateQuery) -> Filter {
    match gate {
        Gate::Permits => Filter::Permits,
        Gate::Refuses => Filter::Refuses,
        Gate::Ask(id) => {
            let gates = world.get_resource::<SlotGates>();
            let function = gates.and_then(|gates| gates.gates.get(usize::from(id.0)));
            function.map_or(Filter::Refuses, |function| function(world, query))
        }
    }
}
