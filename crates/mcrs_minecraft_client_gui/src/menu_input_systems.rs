use bevy::prelude::SystemSet;

/// The systems in `PreUpdate` that feed the input into the open [`crate::menu::Menu`] and write
/// its [`crate::menu_event::MenuEvent`]s. A screen builds its menu before this set and reads the
/// events after it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuInputSystems;
