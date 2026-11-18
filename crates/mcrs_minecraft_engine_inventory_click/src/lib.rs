#![no_std]

mod click_menu;
mod click_slots;
mod end_drags;
mod menu_click_error;
mod menu_click_plugin;
mod menu_clicker;
mod menu_clicks;
mod menu_drag;
mod menu_target_view;
mod menu_targets;
mod start_drags;

pub use menu_click_error::MenuClickError;
pub use menu_click_plugin::MenuClickPlugin;
pub use menu_clicker::MenuClicker;
pub use menu_clicks::MenuClicks;
pub use menu_drag::MenuDrag;
pub use menu_target_view::MenuTargetView;
pub use menu_targets::MenuTargets;
