use bevy_app::App;
use bevy_ecs::change_detection::Mut;
use bevy_ecs::component::Component;
use mcrs_minecraft_item_stack::CustomComponent;

use crate::GateFn;
use crate::GateId;
use crate::RegisterError;
use crate::item_components::ItemComponents;
use crate::slot_gates::SlotGates;

/// Registers custom item component types and slot gates.
pub trait ItemAppExt {
    /// Registers a custom item component type. Register each before the first system that reads
    /// stacks is built: such a system does not run after a later registration.
    fn register_item_component<T: CustomComponent + Component>(
        &mut self,
    ) -> Result<&mut Self, RegisterError>;

    /// Adds a gate function for [`Gate::Ask`](crate::Gate::Ask). More than 65,535 gates give
    /// [`RegisterError::GateTableFull`].
    fn register_slot_gate(&mut self, gate: GateFn) -> Result<GateId, RegisterError>;
}

impl ItemAppExt for App {
    fn register_item_component<T: CustomComponent + Component>(
        &mut self,
    ) -> Result<&mut Self, RegisterError> {
        let world = self.world_mut();
        world.init_resource::<ItemComponents>();
        world
            .try_resource_scope(|world, mut components: Mut<ItemComponents>| {
                components.register::<T>(world)
            })
            .unwrap_or(Ok(()))?;
        Ok(self)
    }

    fn register_slot_gate(&mut self, gate: GateFn) -> Result<GateId, RegisterError> {
        let mut gates = self.world_mut().get_resource_or_init::<SlotGates>();
        gates.register(gate)
    }
}
