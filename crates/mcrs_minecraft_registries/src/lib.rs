#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod diagnostics;
mod id;
mod json;
mod load;
mod order;
mod pack;
mod registry;
mod tags;

pub use diagnostics::{Diagnostic, Diagnostics, Level, Missing, Referrer};
pub use id::{Id, Key, TagId};
pub use load::{
    DataDeclaration, Declaration, IdSeed, IdSetSeed, LoadCtx, RefSeed, Registries, Stage,
    ValueSeed, load,
};
pub use pack::{
    AvailablePack, Features, KnownPack, PackFiles, PackIndex, PackSelection, PackStack,
    StackedPack, pack_stack,
};
pub use registry::{ReadValue, Registry, RegistryType};
pub use tags::{IdSet, Ref, Tags};
