use bevy_ecs::component::Component;

/// A player whose requests [`ArenaRulesPlugin`](crate::ArenaRulesPlugin) denies.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frozen {
    /// The number of requests of the player that the plugin denied since the player became
    /// frozen.
    pub denied: u32,
}
