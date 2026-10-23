#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

mod diagnostic;
mod filter;
mod json;
mod metadata;
mod pack;
mod root;
mod stack;
mod visibility;

pub use diagnostic::{Level, PackDiagnostic};
pub use json::{JsonError, read_json};
pub use pack::{
    AvailablePack, Features, KnownPack, PackFile, PackFiles, PackIndex, PackSelection, PackStack,
    StackedPack, VANILLA_PACK,
};
pub use root::{
    DirectoryEntry, DirectoryPacks, PACKS_ROOT, PackSource, ReadError, RootPacks, VANILLA_ROOT,
    below_vanilla_root,
};
pub use stack::pack_stack;
pub use visibility::Visibility;
