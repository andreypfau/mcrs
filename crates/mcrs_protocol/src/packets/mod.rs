//! All of Minecraft's network packets.
//!
//! Packets are grouped in submodules according to the protocol stage they're
//! used in.

pub mod common;
pub mod status;
pub mod ping;
pub mod login;
pub mod cookie;
pub mod intent;
pub mod configuration;
pub mod game;