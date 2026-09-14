pub mod app;
pub mod file;
pub mod registry;
pub mod tag_ref;

pub use app::{TagLoadersSettled, TagPhase, TagRegistryAppExt};
pub use file::{TagEntry, TagFile, TagFileLoader, TagFileSettings};
pub use registry::{
    DynTagLoader, DynTagRegistry, TagLoader, TagRegistry, TagSource, resolve_tag_file,
    resolve_tag_file_ordered,
};
pub use tag_ref::TagRef;
