use bevy_ecs::component::Component;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::ItemComponent;

/// The explicit removals of prototype components whose type has a neutral value.
///
/// A stack has this component only when it has such a removal. A removal of another type is the
/// absence of the component.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Removed {
    bits: u32,
}

impl Removed {
    #[must_use]
    pub fn contains<T: ItemComponent>(&self) -> bool {
        T::builtin_key().is_some_and(|key| self.bits & bit(key) != 0)
    }

    pub(crate) fn of(key: BuiltinKey) -> Self {
        Self { bits: bit(key) }
    }

    pub(crate) fn bits(self) -> u32 {
        self.bits
    }

    pub(crate) fn with(self, key: BuiltinKey) -> Self {
        Self {
            bits: self.bits | bit(key),
        }
    }

    pub(crate) fn without(self, key: BuiltinKey) -> Self {
        Self {
            bits: self.bits & !bit(key),
        }
    }
}

fn bit(key: BuiltinKey) -> u32 {
    u32::try_from(key.index())
        .ok()
        .and_then(|index| 1u32.checked_shl(index))
        .unwrap_or(0)
}
