//! The settings of a server run that the `MCRS_` environment variables set, read in one place for
//! the server binary and the integrated server of the client.

#![no_std]

extern crate alloc;

mod server_settings;
mod setting_error;

pub use server_settings::ServerSettings;
pub use setting_error::SettingError;
