use bevy_app::App;
use bevy_ecs::change_detection::Mut;
use bevy_ecs::component::Component;
use mcrs_minecraft_item_stack::CustomComponent;

use crate::GateFn;
use crate::GateId;
use crate::RegisterError;
use crate::RequestSource;
use crate::item_components::ItemComponents;
use crate::request_sources::RequestSources;
use crate::slot_gates::SlotGates;

pub trait ItemAppExt {
    fn register_item_component<T: CustomComponent + Component>(
        &mut self,
    ) -> Result<&mut Self, RegisterError>;

    fn register_slot_gate(&mut self, gate: GateFn) -> Result<GateId, RegisterError>;

    fn allocate_request_source(&mut self) -> Result<RequestSource, RegisterError>;
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

    fn allocate_request_source(&mut self) -> Result<RequestSource, RegisterError> {
        let mut sources = self.world_mut().get_resource_or_init::<RequestSources>();
        sources.allocate()
    }
}
