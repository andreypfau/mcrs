//! Resource locations, the keys built on them and registry ids.
#![no_std]

extern crate alloc;

mod id;
mod registry_key;
mod resource_key;
mod resource_key_buf;
mod resource_location;
mod resource_location_buf;
mod tag_key;
mod tag_key_buf;

pub use id::{Id, NarrowError, TagId};
pub use registry_key::{Registered, RegistryKey, RegistryValue};
pub use resource_key::ResourceKey;
pub use resource_key_buf::ResourceKeyBuf;
pub use resource_location::{InvalidResourceLocation, ResourceLocation, parse_parts};
pub use resource_location_buf::ResourceLocationBuf;
pub use tag_key::TagKey;
pub use tag_key_buf::TagKeyBuf;
