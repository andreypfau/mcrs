use alloc::boxed::Box;

use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::Neutral;

/// The components that each stack of one kind has before its patch.
#[derive(Clone, Debug, PartialEq)]
pub struct Prototype {
    components: Box<[Builtin]>,
}

impl Prototype {
    pub(crate) fn new(components: Box<[Builtin]>) -> Self {
        Self { components }
    }

    /// The component of type `T`, or `None` when the prototype has none. A custom type gives
    /// `None`: no prototype has a custom component.
    #[must_use]
    pub fn get<T: ItemComponent>(&self) -> Option<&T> {
        self.components.iter().find_map(T::from_builtin)
    }

    /// Each component, in the order of the bake.
    #[doc(hidden)]
    #[must_use]
    pub fn components(&self) -> &[Builtin] {
        &self.components
    }

    /// True when the component of type `T` is the neutral value of `T`.
    #[must_use]
    pub fn is_neutral<T: Neutral>(&self) -> bool {
        self.get::<T>() == Some(T::NEUTRAL)
    }
}
