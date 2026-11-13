use bevy::prelude::SystemSet;

/// The systems in `PreUpdate` through which the menu screens write the
/// [`mcrs_minecraft_client_options::ClientOptions`]. A system that follows the options in the
/// same frame runs after this set.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuOptionsSystems;
