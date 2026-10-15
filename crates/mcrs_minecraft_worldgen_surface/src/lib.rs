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

mcrs_minecraft_registries::read_with_serde!(
    crate::proto::MaterialRuleHolder,
    crate::proto::MaterialConditionHolder
);
