use bevy_app::App;
use bevy_ecs::change_detection::Mut;
use bevy_ecs::component::Component;
use mcrs_minecraft_item_stack::CustomComponent;

use crate::RegisterError;
use crate::item_components::ItemComponents;

/// Registers custom item component types. Register each before the first system that reads
/// stacks is built: such a system does not run after a later registration.
pub trait ItemAppExt {
    fn register_item_component<T: CustomComponent + Component>(
        &mut self,
    ) -> Result<&mut Self, RegisterError>;
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
}
