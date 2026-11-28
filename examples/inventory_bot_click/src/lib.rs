//! A headless bot that joins a server and clicks in its inventory menu.

mod click_script;
mod click_script_plugin;
mod click_step;
mod headless_client_plugin;
mod join_server_plugin;

pub use click_script::ClickScript;
pub use click_script_plugin::ClickScriptPlugin;
pub use click_step::ClickStep;
pub use headless_client_plugin::HeadlessClientPlugin;
pub use join_server_plugin::JoinServerPlugin;
