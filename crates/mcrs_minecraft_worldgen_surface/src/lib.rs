#![expect(
    clippy::indexing_slicing,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
#[rustfmt::skip]
pub mod keys;
pub mod compile;
pub mod eval;
#[cfg(test)]
mod oracle;
pub mod proto;

pub use compile::{MaterialInputs, SurfaceNoise};
pub use eval::{MaterialEval, MaterialScratch, NO_WATER, SettledState};
pub use proto::{MaterialConditionHolder, MaterialRuleHolder};

mcrs_minecraft_registries::resolve_as_is!(
    crate::proto::MaterialRuleHolder,
    crate::proto::MaterialConditionHolder
);
