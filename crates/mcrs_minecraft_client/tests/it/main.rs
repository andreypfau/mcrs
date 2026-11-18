#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod boot;
#[cfg(feature = "singleplayer")]
mod container_menus;
#[cfg(feature = "singleplayer")]
mod creative_actions;
#[cfg(feature = "singleplayer")]
mod creative_screen_clicks;
#[cfg(feature = "singleplayer")]
mod local_server;
#[cfg(feature = "singleplayer")]
mod local_world;
#[cfg(feature = "singleplayer")]
mod menu_clicks;
#[cfg(feature = "singleplayer")]
mod screen_clicks;
mod session;
mod sky;
#[cfg(feature = "singleplayer")]
mod stacks;
#[cfg(feature = "singleplayer")]
mod world_list;
