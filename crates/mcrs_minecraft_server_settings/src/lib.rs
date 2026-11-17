#![no_std]

extern crate alloc;

mod server_settings;
mod setting_error;

pub use server_settings::ServerSettings;
pub use setting_error::SettingError;
