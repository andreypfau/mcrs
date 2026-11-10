use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::Kind;

/// The kind of a stack of `item`: its protocol id in the item registry.
#[must_use]
pub const fn item_kind(item: Item) -> Kind {
    Kind(item as u16)
}
