use core::fmt;
use std::format;
use std::string::String;

pub fn unknown_entry(registry: impl fmt::Display, location: impl fmt::Display) -> String {
    format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {location}")
}

pub fn unknown_id(registry: impl fmt::Display, id: u16) -> String {
    format!("registry {registry} has no id {id}")
}
