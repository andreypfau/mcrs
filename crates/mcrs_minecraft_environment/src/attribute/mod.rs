mod attribute_map;
mod attribute_value;
mod color;
pub mod id;
mod kind;
mod mob_spawns;
mod operation;
mod value;

pub use attribute_map::{AttributeError, ModifierEntry, ModifierMap};
pub use attribute_value::{AttributeValue, Interpolation, default_value};
pub use color::{argb_hex, rgb_hex};
pub use kind::{AttributeInfo, AttributeType, Bounds, info, is_syncable};
pub use mob_spawns::{MobSpawnSettings, SpawnCost};
pub use operation::{Argument, ArgumentKind, Operation};
pub use value::*;

pub(crate) use attribute_map::read_argument;
pub(crate) use attribute_value::read_float;
