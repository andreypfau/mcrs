#![no_std]

extern crate alloc;

mod death_message;
mod player_death_set;
mod player_vitals_plugin;
mod send_health;
mod send_kill;
mod signal_hits;

pub use death_message::DeathMessage;
pub use player_death_set::PlayerDeathSet;
pub use player_vitals_plugin::PlayerVitalsPlugin;
