use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_table::Prototype;

use crate::Removed;

/// The effective component of one type of a stack, with its relation to the prototype.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComponentState<'a, T> {
    /// The value of the prototype, or the neutral value where the prototype has none.
    Default(&'a T),
    /// A value other than that of the prototype.
    Overridden(&'a T),
    /// The prototype has the type and the stack removed it.
    Removed,
    /// Neither the prototype nor the stack has the type.
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
