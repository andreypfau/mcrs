#[rustfmt::skip]
pub mod activity_keys;
pub mod attribute_id;
#[rustfmt::skip]
pub mod environment_attribute_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod timeline_keys;
#[rustfmt::skip]
pub mod timeline_tag_keys;
#[rustfmt::skip]
pub mod world_clock_keys;

mod attribute_map;
mod attribute_value;
mod clock_markers;
mod clock_time;
mod color;
mod easing;
mod kind;
mod mob_spawns;
mod operation;
mod time_marker;
mod timeline;
mod track;
mod value;
#[cfg(feature = "bevy")]
mod world_clock_plugin;
mod world_clocks;

pub use attribute_map::{AttributeError, ModifierEntry, ModifierMap, read_argument};
pub use attribute_value::{AttributeValue, Interpolation, default_value};
pub use clock_markers::{ClockTimeMarker, ClockTimeMarkers};
pub use clock_time::{ClockEdit, ClockTime};
pub use color::{argb_hex, rgb_hex};
pub use easing::{Easing, NamedEasing};
pub use kind::{AttributeInfo, AttributeType, Bounds, info, is_syncable};
pub use mob_spawns::{MobSpawnSettings, SpawnCost};
pub use operation::{Argument, ArgumentKind, Operation};
pub use time_marker::TimeMarker;
pub use timeline::{NetworkTimeline, Timeline};
pub use track::{Keyframe, Track};
pub use value::*;
#[cfg(feature = "bevy")]
pub use world_clock_plugin::{WorldClockPlugin, WorldClockSystems};
pub use world_clocks::{AdvanceTime, WorldClock, WorldClocks};

pub(crate) use attribute_value::read_float;
