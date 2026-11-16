use bevy_ecs::component::Component;

/// A player whose requests [`ArenaRulesPlugin`](crate::ArenaRulesPlugin) denies.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frozen;
