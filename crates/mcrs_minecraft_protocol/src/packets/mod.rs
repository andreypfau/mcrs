//! All of Minecraft's network packets.
//!
//! Packets are grouped in submodules according to the protocol stage they're
//! used in.

pub mod common;
pub mod configuration;
pub mod cookie;
pub mod game;
pub mod intent;
pub mod login;
pub mod ping;
pub mod status;
