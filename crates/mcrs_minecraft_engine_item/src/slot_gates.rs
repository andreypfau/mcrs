use alloc::vec::Vec;

use bevy_ecs::resource::Resource;

use crate::GateFn;
use crate::GateId;
use crate::RegisterError;

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

    pub(crate) fn get(&self, id: GateId) -> Option<GateFn> {
        self.gates.get(usize::from(id.0)).copied()
    }
}
