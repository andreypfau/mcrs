#![no_std]

extern crate alloc;

mod close_on_holder_removal;
mod entry_pressed;
mod menu_buttons;
mod menu_closed;
mod menu_commands_ext;
mod menu_of;
mod menu_opened;
mod menu_parts;
mod menu_plugin;
mod menu_title;
mod menus;

pub use entry_pressed::EntryPressed;
pub use menu_buttons::MenuButtons;
pub use menu_closed::MenuClosed;
pub use menu_commands_ext::MenuCommandsExt;
pub use menu_of::MenuOf;
pub use menu_opened::MenuOpened;
pub use menu_parts::MAX_PARTS;
pub use menu_parts::MenuParts;
pub use menu_plugin::MenuPlugin;
pub use menu_title::MenuTitle;
pub use menus::Menus;
