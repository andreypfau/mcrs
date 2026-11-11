use core::marker::PhantomData;

use bevy_ecs::component::Component;
use bevy_ecs::component::ComponentId;
use bevy_ecs::component::Components;
use bevy_ecs::world::World;

/// The id of the component `T` in one world.
pub(crate) struct Column<T> {
    id: ComponentId,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for Column<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Column<T> {}

impl<T> Column<T> {
    /// # Safety
    /// `id` is the id of `T` in each world whose cells are read through the column.
    pub(crate) unsafe fn assume(id: ComponentId) -> Self {
        Self {
            id,
            value: PhantomData,
        }
    }

    pub(crate) fn id(self) -> ComponentId {
        self.id
    }
}

impl<T: Component> Column<T> {
    pub(crate) fn register(world: &mut World) -> Self {
        Self {
            id: world.register_component::<T>(),
            value: PhantomData,
        }
    }

    pub(crate) fn of(components: &Components) -> Option<Self> {
        Some(Self {
            id: components.component_id::<T>()?,
            value: PhantomData,
        })
    }
}
