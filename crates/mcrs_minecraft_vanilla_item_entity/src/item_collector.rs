use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_physics::Aabb;

/// A player that collects item entities into its slots. The host keeps `bounds` and `vehicle`
/// current and removes the component while the player is dead or a spectator (IEN-PCK-001).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ItemCollector {
    /// The network id of the player, which a pickup sends (IEN-NET-004).
    pub id: EntityId,
    pub uuid: u128,
    pub bounds: Aabb,
    /// The box of the vehicle that the player rides, when that vehicle is not removed.
    pub vehicle: Option<Aabb>,
    pub mode: Collect,
}

/// What happens to the items that find no room (IEN-PCK-006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collect {
    /// They stay in the item entity.
    Survival,
    /// They are destroyed, and the pickup takes all items.
    Creative,
}
