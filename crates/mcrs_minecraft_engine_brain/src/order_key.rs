use mcrs_minecraft_brain::OrderKey;
use mcrs_minecraft_entity_key::EntityKey;

/// The order key of the brain for the entity with `key`: its UUID.
#[must_use]
pub fn order_key(key: &EntityKey) -> OrderKey {
    OrderKey::new(key.0)
}
