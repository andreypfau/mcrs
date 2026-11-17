#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
mod registry_id;
mod static_entry;
#[cfg(feature = "std")]
mod static_registry;
mod static_registry_core;
#[cfg(feature = "std")]
mod unknown_entry;

#[cfg(feature = "std")]
pub use registry_id::{decode_registry_id, encode_registry_id};
pub use static_entry::StaticEntry;
#[cfg(feature = "std")]
#[doc(hidden)]
pub use unknown_entry::{unknown_entry, unknown_id};

#[doc(hidden)]
pub use mcrs_minecraft_resource::ResourceLocation as __ResourceLocation;
#[doc(hidden)]
pub use mcrs_minecraft_resource::rl as __rl;
#[cfg(feature = "std")]
#[doc(hidden)]
pub use mcrs_minecraft_wire as __wire;
#[cfg(feature = "std")]
#[doc(hidden)]
pub use serde as __serde;
