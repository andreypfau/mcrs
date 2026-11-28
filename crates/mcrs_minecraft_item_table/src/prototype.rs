use alloc::boxed::Box;

use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::ItemComponent;

#[derive(Clone, Debug, PartialEq)]
pub struct Prototype {
    components: Box<[Builtin]>,
}

impl Prototype {
    pub(crate) fn new(components: Box<[Builtin]>) -> Self {
        Self { components }
    }

    #[must_use]
    pub fn get<T: ItemComponent>(&self) -> Option<&T> {
        self.components.iter().find_map(T::from_builtin)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn components(&self) -> &[Builtin] {
        &self.components
    }
}
