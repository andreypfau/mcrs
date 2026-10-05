// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const ACQUIRABLE_JOB_SITE: TagKey<crate::PointOfInterestType, &'static str> = TagKey::new(rl!("minecraft:acquirable_job_site"));
pub const BEE_HOME: TagKey<crate::PointOfInterestType, &'static str> = TagKey::new(rl!("minecraft:bee_home"));
pub const VILLAGE: TagKey<crate::PointOfInterestType, &'static str> = TagKey::new(rl!("minecraft:village"));
