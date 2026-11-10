use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_item_stack::CustomComponent;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_stack::PatchEntry;
use rustc_hash::FxHasher;

use crate::RowHashing;

/// The operations on the value entries of one component type, as plain `fn` pointers.
///
/// Each operation reads only a value entry of its own type. Any other entry is not a value of
/// the type: it is equal to nothing, has no hash and no clone, and is not neutral.
#[derive(Clone, Copy, Debug)]
pub struct ComponentRow {
    equal: fn(&PatchEntry, &PatchEntry) -> bool,
    hash: fn(&PatchEntry) -> Option<u64>,
    clone: fn(&PatchEntry) -> Option<PatchEntry>,
    is_neutral: fn(&PatchEntry) -> bool,
    hashing: RowHashing,
}

impl ComponentRow {
    pub(crate) fn builtin<T: ItemComponent>() -> Self {
        Self {
            equal: equal_builtin::<T>,
            hash: hash_builtin::<T>,
            clone: clone_builtin::<T>,
            is_neutral: never_neutral,
            hashing: RowHashing::Included,
        }
    }

    pub(crate) fn builtin_with_neutral<T: Neutral>() -> Self {
        Self {
            is_neutral: neutral_builtin::<T>,
            ..Self::builtin::<T>()
        }
    }

    pub(crate) fn custom<C: CustomComponent>(hashing: RowHashing) -> Self {
        Self {
            equal: equal_custom::<C>,
            hash: hash_custom::<C>,
            clone: clone_custom::<C>,
            is_neutral: never_neutral,
            hashing,
        }
    }

    /// True when the two entries are values of this type and equal as typed values.
    #[must_use]
    pub fn equal(&self, left: &PatchEntry, right: &PatchEntry) -> bool {
        (self.equal)(left, right)
    }

    /// The hash of the typed value of `entry`. Two equal values have one hash.
    #[must_use]
    pub fn hash(&self, entry: &PatchEntry) -> Option<u64> {
        (self.hash)(entry)
    }

    /// The entry of a clone of the typed value of `entry`.
    #[must_use]
    pub fn clone_entry(&self, entry: &PatchEntry) -> Option<PatchEntry> {
        (self.clone)(entry)
    }

    /// True when `entry` holds the neutral value of a type that has one.
    #[must_use]
    pub fn is_neutral(&self, entry: &PatchEntry) -> bool {
        (self.is_neutral)(entry)
    }

    #[must_use]
    pub fn hashing(&self) -> RowHashing {
        self.hashing
    }
}

fn builtin_value<T: ItemComponent>(entry: &PatchEntry) -> Option<&T> {
    match entry {
        PatchEntry::Value(value) => T::from_builtin(value),
        _ => None,
    }
}

fn custom_value<C: CustomComponent>(entry: &PatchEntry) -> Option<C> {
    match entry {
        PatchEntry::Custom(value) if value.key == C::KEY => C::decode(&value.bytes),
        _ => None,
    }
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = FxHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

fn equal_builtin<T: ItemComponent>(left: &PatchEntry, right: &PatchEntry) -> bool {
    matches!(
        (builtin_value::<T>(left), builtin_value::<T>(right)),
        (Some(left), Some(right)) if left == right
    )
}

fn hash_builtin<T: ItemComponent>(entry: &PatchEntry) -> Option<u64> {
    builtin_value::<T>(entry).map(hash_of)
}

fn clone_builtin<T: ItemComponent>(entry: &PatchEntry) -> Option<PatchEntry> {
    builtin_value::<T>(entry).map(|value| value.clone().into_entry())
}

fn neutral_builtin<T: Neutral>(entry: &PatchEntry) -> bool {
    builtin_value::<T>(entry) == Some(T::NEUTRAL)
}

fn never_neutral(_: &PatchEntry) -> bool {
    false
}

fn equal_custom<C: CustomComponent>(left: &PatchEntry, right: &PatchEntry) -> bool {
    matches!(
        (custom_value::<C>(left), custom_value::<C>(right)),
        (Some(left), Some(right)) if left == right
    )
}

fn hash_custom<C: CustomComponent>(entry: &PatchEntry) -> Option<u64> {
    custom_value::<C>(entry).map(|value| hash_of(&value))
}

fn clone_custom<C: CustomComponent>(entry: &PatchEntry) -> Option<PatchEntry> {
    custom_value::<C>(entry).map(ItemComponent::into_entry)
}
