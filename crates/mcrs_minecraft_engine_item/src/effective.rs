use bevy_ecs::component::Component;
use bevy_ecs::query::QueryData;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_table::ItemTable;

use crate::ComponentState;
use crate::Removed;

/// The effective component of type `T` of a stack. It reads only `T`, [`Removed`] and [`Kind`].
#[derive(QueryData)]
pub struct Effective<T: ItemComponent + Component> {
    stored: Option<&'static T>,
    removed: Option<&'static Removed>,
    kind: &'static Kind,
}

impl<'w, T: ItemComponent + Component> EffectiveItem<'w, '_, T> {
    #[must_use]
    pub fn get(&self) -> Option<&'w T> {
        effective(self.stored, self.removed)
    }

    #[must_use]
    pub fn or_neutral(&self) -> &'w T
    where
        T: Neutral,
    {
        self.get().unwrap_or(T::NEUTRAL)
    }

    #[must_use]
    pub fn state(&self, table: &ItemTable) -> ComponentState<'w, T> {
        ComponentState::of(self.get(), self.removed, table.prototype(*self.kind))
    }
}

/// The effective value from the stored component and the removals of a stack: the stored value,
/// else the neutral value of `T` when `removed` has no removal of `T`.
#[must_use]
pub fn effective<'a, T: ItemComponent + 'static>(
    stored: Option<&'a T>,
    removed: Option<&Removed>,
) -> Option<&'a T> {
    stored.or_else(|| {
        if removed.is_some_and(Removed::contains::<T>) {
            None
        } else {
            T::neutral()
        }
    })
}
