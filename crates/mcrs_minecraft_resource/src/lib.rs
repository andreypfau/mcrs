//! Resource locations, the keys built on them and registry ids.
#![no_std]

extern crate alloc;

mod id;
mod registry_key;
mod resource_key;
mod resource_location;
mod tag_key;

pub use id::{Id, NarrowError, TagId};
pub use registry_key::{Registered, RegistryKey, RegistryValue};
pub use resource_key::{ResourceKey, ResourceKeyBuf};
pub use resource_location::{
    InvalidResourceLocation, ResourceLocation, ResourceLocationBuf, StaticResourceLocation,
    parse_parts,
};
pub use tag_key::{TagKey, TagKeyBuf};
