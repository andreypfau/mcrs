#![cfg_attr(
    test,
    expect(clippy::disallowed_types, reason = "test input is written as JSON")
)]

mod assembly;
pub mod by_name;
pub mod dispatch;
pub mod entries;
pub mod holder;
pub mod holder_list;
pub mod holder_set;
pub mod load;
pub mod lookup;
pub mod names;
pub mod registry;
pub mod report;
mod resolver;
pub mod set;
pub mod static_report;
pub mod tags;

pub use assembly::AssemblyError;
pub use entries::Entries;
pub use holder::*;
pub use holder_list::HolderList;
pub use holder_set::{AlwaysList, HolderSet, skip_sets, skipping_sets};
pub use load::{
    PACKS_ROOT, Parts, VANILLA_PACK, VANILLA_ROOT, WorldRegistries, below_vanilla_root,
};
pub use lookup::{ChainLookup, LookupIndex, NoRegistries, RegistryLookup};
pub use names::NameTable;
pub use registry::{Registry, RegistryError, UnknownEntry};
pub use report::LoadReport;
pub use resolver::{Checked, check_in_scope, name_in_scope};
pub use set::{RegistrySet, ScopeError};
pub use tags::{TagProblem, TagRules, TagTable, Tags, build_tags, number_tags};
