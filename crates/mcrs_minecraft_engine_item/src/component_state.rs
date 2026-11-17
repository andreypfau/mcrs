use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_table::Prototype;

use crate::Removed;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComponentState<'a, T> {
    Default(&'a T),
    Overridden(&'a T),
    Removed,
    Absent,
}

impl<'a, T: ItemComponent + 'static> ComponentState<'a, T> {
    pub(crate) fn of(
        value: Option<&'a T>,
        removed: Option<&Removed>,
        prototype: Option<&'static Prototype>,
    ) -> Self {
        let base = prototype.and_then(Prototype::get::<T>).or_else(T::neutral);
        match value {
            Some(value) if Some(value) == base => Self::Default(value),
            Some(value) => Self::Overridden(value),
            None if removed.is_some_and(Removed::contains::<T>) || base.is_some() => Self::Removed,
            None => Self::Absent,
        }
    }
}
