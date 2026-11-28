#![cfg_attr(not(test), deny(clippy::arithmetic_side_effects))]

mod diagnostic;
mod filter;
mod json;
mod metadata;
mod pack;
mod pack_conflict;
mod root;
mod selected_stack;
mod stack;

pub use diagnostic::{Level, PackDiagnostic};
pub use json::{JsonError, read_json};
pub use pack::{
    AvailablePack, Features, KnownPack, PackContent, PackFile, PackFiles, PackIndex, PackSelection,
    PackStack, StackedPack, VANILLA_PACK,
};
pub use pack_conflict::PackConflict;
pub use root::{
    DirectoryEntry, DirectoryPacks, PACKS_ROOT, PackSource, REPOSITORY_PACKS, ReadError, RootPacks,
    VANILLA_ROOT, below_vanilla_root, read_vanilla,
};
pub use selected_stack::SelectedStack;
pub use stack::pack_stack;
